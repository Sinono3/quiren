use osstrtools::OsStrConcat;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

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

fn main() -> Result<(), main_error::MainError> {
    let arg = env::args().nth(1);
    let dir = arg
        .filter(|a| !a.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap();

    // TODO: clap arguments
    // TODO: add retry until correct option

    Ok(quiren(dir)?)
}

fn quiren(dir: PathBuf) -> Result<(), QuirenError> {
    let mut entries: Vec<_> = dir.read_dir()?.map(|e| e.unwrap()).collect();

    entries.sort_by_key(|e| e.file_name());

    let text: OsString = entries
        .iter()
        .map(|e| e.file_name())
        .collect::<Vec<OsString>>()
        .concat("\n");

    let text = text.to_string_lossy().into_owned();

    let edited = edit::edit(&text)?;

    // check if linecount = entry count
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

    for (i, line) in edited
        .lines()
        .enumerate()
        // Only rename files with modified names
        .filter(|(i, line)| {
            let name = OsStr::new(line);
            name != entries[*i].file_name()
        })
    {
        let mut new_path = dir.clone();
        new_path.push(line);
        fs::rename(&entries[i].path(), new_path)?;
    }

    Ok(())
}
