use osstrtools::OsStrConcat;
use question::{Answer, Question};
use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use thiserror::Error;

mod util;
use util::tmpfile;

const HELP: &str = "\
Usage: quiren [options] [dir]

Modes:
    <default>           Rename mode: Rename files modified in the editor
    -d, --delete-mode   Delete mode: Delete files removed in the editor

Options:
    -h, --help          Prints help information
    -r, --retry         Re-enters the editor after an error
    -n, --dry-run       Show changes and ask for confirmation
    -t, --trash         Trash files instead of deleting them
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Rename,
    Delete
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Args {
    mode: Mode,
    dryrun: bool,
    trash: bool,
}

fn main() -> Result<(), main_error::MainError> {
    let mut pargs = pico_args::Arguments::from_env();

    let help = pargs.contains(["-h", "--help"]);
    let retry = pargs.contains(["-r", "--retry"]);
    let dryrun = pargs.contains(["-n", "--dry-run"]);
    let trash = pargs.contains(["-t", "--trash"]);

    let delete_mode = pargs.contains(["-d", "--delete-mode"]);

    let mode = if delete_mode {
        Mode::Delete
    } else {
        // The default behaviour is to rename files.
        Mode::Rename
    };

    let dir: PathBuf = pargs
        .free_from_str()
        .or_else(|_| env::current_dir())
        .unwrap();

    if help {
        print!("{}", HELP);
        return Ok(());
    }

    if retry {
        use std::io::Read;
        let mut stdin = std::io::stdin();

        while let Err(err) = quiren(&dir, Args { mode, dryrun, trash }) {
            eprintln!("Error: {}", err);
            eprintln!("Press enter to retry");

            let _ = stdin.read(&mut [0u8]);
        }
        return Ok(());
    }

    Ok(quiren(&dir, Args { mode, dryrun, trash })?)
}

#[derive(Error, Debug)]
pub enum QuirenError {
    #[error("the entry '{0}' was assigned an empty name")]
    EmptyName(String),
    #[error("the filename {0} is duplicated")]
    DuplicateName(String),
    #[error("lines cannot be deleted or added: {0} -> {1}")]
    EntryCountMismatch(usize, usize),
    #[error("the filename {1} will be overwritten by {0}")]
    Overwrite(PathBuf, PathBuf),
    #[error("Couldn't allocate auxiliary tempfile")]
    Tempfile,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("error when trashing: {0}")]
    TrashError(#[from] trash::Error),
}

pub fn quiren(dir: &Path, args: Args) -> Result<(), QuirenError> {
    let mut entries: Vec<_> = dir.read_dir()?.map(|e| e.unwrap()).collect();

    entries.sort_by_key(|e| e.file_name());

    let text = entries
        .iter()
        .map(|e| e.file_name())
        .concat("\n")
        .to_string_lossy()
        .into_owned();

    let mut edited = edit::edit(&text)?;
    let mut changes = Vec::new();

    // We add the changes
    match args.mode {
        Mode::Rename => changes.extend(extract_renames(&edited, &dir, &entries)?),
        Mode::Delete => changes.extend(extract_deletions(&edited, &entries)?),
    }

    if args.dryrun {
        loop {
            if confirm_changes(&changes, args.trash) {
                break;
            }

            edited = edit::edit(&edited)?;
            changes.clear();

            match args.mode {
                Mode::Rename => changes.extend(extract_renames(&edited, &dir, &entries)?),
                Mode::Delete => changes.extend(extract_deletions(&edited, &entries)?),
            }
        }
    }

    // Files that have been moved to a tempfile
    let mut moved_to_tempfile: HashMap<&Path, PathBuf> = HashMap::new();

    // Perform the filesystem operations.
    for change in changes.iter() {
        match change {
            Change::Rename(a, b) => {
                // Check if a file already exists at the new name
                if b.exists() {
                    // Check if `b` will also be renamed or deleted
                    let b_in_changes = changes
                        .iter()
                        .find(|c| match c {
                            Change::Rename(x, _) => x == b,
                            Change::Delete(x) => x == b,
                        })
                        .is_some();

                    // If not, then we cannot perform the renames
                    // without `a` overwriting `b`
                    if !b_in_changes {
                        return Err(QuirenError::Overwrite(a.to_path_buf(), b.to_path_buf()));
                    }

                    let aux = tmpfile(b.parent().unwrap())?;
                    fs::rename(b, &aux)?;
                    moved_to_tempfile.insert(b, aux);
                }

                // Check if `a` was moved to an auxiliary tempfile
                let a = if let Some(temp) = moved_to_tempfile.get(a.as_path()) {
                    temp
                } else {
                    &a
                };

                fs::rename(a, b)?
            }
            Change::Delete(a) if args.trash => trash::delete(a)?,
            Change::Delete(a) => fs::remove_file(a)?,
        }
    }

    Ok(())
}

enum Change {
    // Rename: file_a -> file_b
    Rename(PathBuf, PathBuf),
    // Delete: file_a
    Delete(PathBuf),
}

/// Returns an iterator with all the renames found.
fn extract_renames<'a>(
    edited: &'a str,
    dir: &'a Path,
    entries: &'a Vec<DirEntry>,
) -> Result<impl Iterator<Item = Change> + 'a, QuirenError> {
    // Check if linecount = entry count
    let new_count = edited.lines().count();

    if new_count != entries.len() {
        return Err(QuirenError::EntryCountMismatch(entries.len(), new_count));
    }

    // Other checks
    for (i, a) in edited.lines().enumerate() {
        // Check for empty names
        if a.is_empty() {
            return Err(QuirenError::EmptyName(
                entries[i].file_name().to_string_lossy().to_string(),
            ));
        }

        // Check for duplicates
        for (j, b) in edited.lines().enumerate() {
            if i != j && a == b {
                return Err(QuirenError::DuplicateName(a.to_string()));
            }
        }
    }

    let iter = edited
        .lines()
        .enumerate()
        // Only rename files with modified names
        .filter(move |(i, line)| {
            let name = OsStr::new(line);
            name != entries[*i].file_name()
        })
        .map(move |(i, line)| {
            let mut new_path = dir.to_owned();
            new_path.push(line);
            Change::Rename(entries[i].path(), new_path)
        });

    Ok(iter)
}

/// Returns an iterator with all the deletions found.
fn extract_deletions<'a>(
    edited: &'a str,
    entries: &'a Vec<DirEntry>,
) -> Result<impl Iterator<Item = Change> + 'a, QuirenError> {
    // Delete files that have been deleted in the editor and return
    // Managing deletion AND rename is too complex. Users must perform
    // there operations separately

    let r: Vec<OsString> = edited
        .lines()
        .map(OsString::from)
        .collect::<Vec<OsString>>();

    let iter = entries
        .iter()
        .filter(move |existed| !r.contains(&existed.file_name()))
        .map(move |entry| Change::Delete(entry.path()));

    Ok(iter)
}


fn confirm_changes(changes: &[Change], trash: bool) -> bool {
    let delete_action = if trash { "Trash" } else { "Delete" };

    for change in changes {
        match change {
            Change::Rename(a, b) => println!("Rename: {} -> {}", a.display(), b.display()),
            Change::Delete(a) => println!("{}: {}", delete_action, a.display()),
        }
    }


    if changes.is_empty() {
        println!("No changes.");
        return true;
    }

    let answer = Question::new("Confirm ?")
        .default(Answer::YES)
        .show_defaults()
        .confirm();

    answer == Answer::YES
}
