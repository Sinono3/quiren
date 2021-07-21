use osstrtools::OsStrConcat;
use question::{Answer, Question};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const HELP: &str = "\
Usage: quiren [options] [dir]

Options:
    -h, --help      Prints help information
    -r, --retry     Re-enters the editor after an error
    -d, --delete    Delete files removed in the editor
    -n, --dry-run   Show changes and ask for confirmation
";

struct Args {
    delete: bool,
    dryrun: bool,
}

fn main() -> Result<(), main_error::MainError> {
    let mut pargs = pico_args::Arguments::from_env();

    let help = pargs.contains(["-h", "--help"]);
    let retry = pargs.contains(["-r", "--retry"]);
    let delete = pargs.contains(["-d", "--delete"]);
    let dryrun = pargs.contains(["-n", "--dry-run"]);

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

        while let Err(err) = quiren(&dir, Args { delete, dryrun }) {
            eprintln!("Error: {}", err);
            eprintln!("Press enter to retry");

            let _ = stdin.read(&mut [0u8]);
        }
        return Ok(());
    }

    Ok(quiren(&dir, Args { delete, dryrun })?)
}

#[derive(Error, Debug)]
enum QuirenError {
    #[error("the entry '{0}' was assigned an empty name")]
    EmptyName(String),
    #[error("the filename {0} is duplicated")]
    DuplicateName(String),
    #[error("lines cannot be deleted or added: {0} -> {1}")]
    EntryCountMismatch(usize, usize),
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),
}

fn quiren(dir: &Path, args: Args) -> Result<(), QuirenError> {
    let mut entries: Vec<_> = dir.read_dir()?.map(|e| e.unwrap()).collect();

    entries.sort_by_key(|e| e.file_name());

    let entries_name = entries
        .iter()
        .map(|e| e.file_name())
        .collect::<Vec<OsString>>();
    let text = entries_name.clone().concat("\n");

    let text = text.to_string_lossy().into_owned();

    let mut edited = edit::edit(&text)?;

    // check if linecount = entry count
    let new_count = edited.lines().count();

    if new_count != entries.len() && !args.delete {
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

    // Delete files that have been deleted in the editor and return
    // Managing deletion AND rename is too complex. Users must perform
    // there operations separately
    if args.delete {
        let r: Vec<OsString> = edited
            .lines()
            .map(OsString::from)
            .collect::<Vec<OsString>>();
        if args.delete {
            let to_delete: Vec<_> = entries_name
                .iter()
                .filter(|existed| !r.contains(existed))
                .collect();

            for d in to_delete {
                let mut new_path = dir.to_owned();
                new_path.push(d);
                fs::remove_file(new_path).unwrap();
            }
        }

        return Ok(());
    }

    if args.dryrun {
        loop {
            for (i, line) in edited
                .lines()
                .enumerate()
                // Only rename files with modified names
                .filter(|(i, line)| {
                    let name = OsStr::new(line);
                    name != entries[*i].file_name()
                })
            {
                let mut new_path = dir.to_owned();
                new_path.push(line);
                println!(
                    "{} -> {}",
                    &entries[i].path().to_str().unwrap(),
                    new_path.to_str().unwrap(),
                );
            }
            let answer = Question::new("Confirm ?")
                .default(Answer::YES)
                .show_defaults()
                .confirm();
            if answer == Answer::NO {
                edited = edit::edit(&edited)?;
            } else {
                break;
            }
        }
    }

    for (i, line) in edited
        .lines()
        .enumerate()
        // Only rename files with modified names
        .filter(|(i, line)| {
            let name = OsStr::new(line);
            name != entries[*i].file_name()
        })
    {
        let mut new_path = dir.to_owned();
        new_path.push(line);
        fs::rename(&entries[i].path(), new_path)?;
    }

    Ok(())
}
