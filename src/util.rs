use crate::QuirenError;
use rand::{distributions::Alphanumeric, rngs::SmallRng};
use rand::{Rng, SeedableRng};
use std::cell::UnsafeCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const TEMPFILE_MAX_RETRIES: usize = 20;
const TEMPFILE_NAME_LENGTH: usize = 8;

thread_local! {
    static THREAD_RNG: UnsafeCell<SmallRng> = UnsafeCell::new(SmallRng::from_entropy());
}

// Borrowed from `tempfile` crate.
pub fn tmpfile(path: &Path) -> Result<PathBuf, QuirenError> {
    for _ in 0..TEMPFILE_MAX_RETRIES {
        let mut buf = OsString::with_capacity(TEMPFILE_NAME_LENGTH);

        // Push each character in one-by-one. Unfortunately, this is the only
        // safe(ish) simple way to do this without allocating a temporary
        // String/Vec.
        THREAD_RNG.with(|rng| unsafe {
            (&mut *rng.get())
                .sample_iter(&Alphanumeric)
                .take(TEMPFILE_NAME_LENGTH)
                .for_each(|b| buf.push(std::str::from_utf8_unchecked(&[b as u8])))
        });

        let path = path.join(Path::new(&buf));

        if !path.exists() {
            return Ok(path.to_owned());
        }
    }

    Err(QuirenError::Tempfile)
}
