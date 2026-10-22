//! The speech recognition model on this device (D-23).
//!
//! The user downloads the model archive in their browser and picks the file;
//! Mossbook never fetches it. The archive is accepted only if its size and
//! SHA-256 match the pinned values; only the files the recognizer needs are
//! unpacked, by name, into a staging folder that is renamed into place once
//! everything checks out. The model is not user data: it is stored in plain
//! form, outside the journal, backups and sync.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bzip2::read::MultiBzDecoder;
use sha2::{Digest, Sha256};

use super::TranscribeError;

/// A model archive Mossbook accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    /// Folder inside the archive, and the folder it is unpacked to.
    pub name: &'static str,
    /// Where the user's browser downloads it from.
    pub url: &'static str,
    pub archive_bytes: u64,
    /// SHA-256 of the whole archive, lowercase hex.
    pub archive_sha256: &'static str,
    /// The files the recognizer needs, with their sizes.
    pub files: &'static [(&'static str, u64)],
}

/// NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0), int8, as packaged by sherpa-onnx.
pub const PARAKEET_V3: ModelSpec = ModelSpec {
    name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
    archive_bytes: 487_170_055,
    archive_sha256: "5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf",
    files: &[
        (ENCODER, 652_184_281),
        (DECODER, 11_845_275),
        (JOINER, 6_355_277),
        (TOKENS, 93_939),
    ],
};

pub(super) const ENCODER: &str = "encoder.int8.onnx";
pub(super) const DECODER: &str = "decoder.int8.onnx";
pub(super) const JOINER: &str = "joiner.int8.onnx";
pub(super) const TOKENS: &str = "tokens.txt";

/// Folder for models, next to the vault.
pub const DIR_NAME: &str = "transcription";

/// The model folder of `spec` under `root`.
pub fn model_dir(root: &Path, spec: &ModelSpec) -> PathBuf {
    root.join(spec.name)
}

fn staging_dir(root: &Path, spec: &ModelSpec) -> PathBuf {
    root.join(format!("{}.partial", spec.name))
}

/// Whether the model is in place: every file present with its exact size.
/// The content was checked when it was imported.
pub fn is_installed(root: &Path, spec: &ModelSpec) -> bool {
    let dir = model_dir(root, spec);
    spec.files.iter().all(|(name, bytes)| {
        fs::metadata(dir.join(name)).is_ok_and(|m| m.is_file() && m.len() == *bytes)
    })
}

/// Removes the model (and any unfinished import). Turns transcription off.
pub fn remove(root: &Path, spec: &ModelSpec) -> io::Result<()> {
    for dir in [staging_dir(root, spec), model_dir(root, spec)] {
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Imports the model from the archive the user picked. `progress` receives the
/// number of archive bytes read so far; `cancel` stops the import between
/// blocks. Nothing is left behind unless the whole archive checks out.
pub fn install(
    root: &Path,
    spec: &ModelSpec,
    archive: impl Read,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), TranscribeError> {
    let staging = staging_dir(root, spec);
    let result = unpack(&staging, spec, archive, cancel, progress).and_then(|()| {
        let target = model_dir(root, spec);
        remove_if_present(&target)?;
        fs::rename(&staging, &target)?;
        Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn remove_if_present(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn unpack(
    staging: &Path,
    spec: &ModelSpec,
    archive: impl Read,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), TranscribeError> {
    remove_if_present(staging)?;
    fs::create_dir_all(staging)?;
    let read = AtomicU64::new(0);
    let counted = Checked {
        inner: archive,
        hash: Sha256::new(),
        read: &read,
        limit: spec.archive_bytes,
    };
    let mut tar = tar::Archive::new(MultiBzDecoder::new(counted));
    let mut found = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    for entry in tar.entries().map_err(invalid)? {
        let mut entry = entry.map_err(invalid)?;
        let path = entry.path().map_err(invalid)?.into_owned();
        let Some((name, bytes)) = wanted(spec, &path) else {
            continue; // examples and folders in the archive are skipped
        };
        if !entry.header().entry_type().is_file() || entry.size() != bytes || found.contains(&name)
        {
            return Err(TranscribeError::ModelInvalid);
        }
        let mut out = BufWriter::new(File::create(staging.join(name))?);
        let mut left = bytes;
        while left > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Err(TranscribeError::Cancelled);
            }
            let n = entry.read(&mut buf).map_err(invalid)?;
            if n == 0 {
                return Err(TranscribeError::ModelInvalid);
            }
            out.write_all(&buf[..n])?;
            left = left.saturating_sub(n as u64);
            progress(read.load(Ordering::Relaxed));
        }
        out.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        found.push(name);
    }
    // Read to the very end, so the checksum covers the whole file.
    let mut rest = tar.into_inner();
    io::copy(&mut rest, &mut io::sink()).map_err(invalid)?;
    let mut checked = rest.into_inner();
    io::copy(&mut checked, &mut io::sink()).map_err(invalid)?;
    let digest = checked.hash.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if read.load(Ordering::Relaxed) != spec.archive_bytes
        || hex != spec.archive_sha256
        || found.len() != spec.files.len()
    {
        return Err(TranscribeError::ModelInvalid);
    }
    progress(spec.archive_bytes);
    Ok(())
}

/// The needed file `path` names, if any: exactly `<spec.name>/<file>`.
fn wanted(spec: &ModelSpec, path: &Path) -> Option<(&'static str, u64)> {
    let mut parts = path.components().filter(|c| *c != Component::CurDir);
    let (Some(Component::Normal(dir)), Some(Component::Normal(file)), None) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if dir != spec.name {
        return None;
    }
    spec.files.iter().copied().find(|(name, _)| file == *name)
}

/// Any error while reading or unpacking means this is not the archive we expect.
fn invalid(_: io::Error) -> TranscribeError {
    TranscribeError::ModelInvalid
}

/// Hashes and counts what is read, and refuses to read past the expected size
/// (a wrong, larger file is turned down without reading it all).
struct Checked<'a, R> {
    inner: R,
    hash: Sha256,
    read: &'a AtomicU64,
    limit: u64,
}

impl<R: Read> Read for Checked<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hash.update(&buf[..n]);
        let total = self.read.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
        if total > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "archive too large",
            ));
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bzip2::Compression;
    use bzip2::write::BzEncoder;
    use tempfile::TempDir;

    use super::*;

    const FILES: &[(&str, u64)] = &[("model.onnx", 3000), ("tokens.txt", 5)];

    /// A small archive shaped like the real one, and a spec that accepts it.
    fn archive(extra: &[(&str, &[u8])]) -> (Vec<u8>, ModelSpec) {
        let mut tar = tar::Builder::new(Vec::new());
        let mut add = |path: &str, data: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, data).unwrap();
        };
        add("synthetic-model/model.onnx", &[7u8; 3000]);
        add("synthetic-model/test_wavs/en.wav", b"RIFF synthetic");
        add("synthetic-model/tokens.txt", b"a 0\n\n");
        for (path, data) in extra {
            add(path, data);
        }
        let tar = tar.into_inner().unwrap();
        let mut bz = BzEncoder::new(Vec::new(), Compression::fast());
        bz.write_all(&tar).unwrap();
        let bytes = bz.finish().unwrap();
        let hex = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let spec = ModelSpec {
            name: "synthetic-model",
            url: "https://example.invalid/model.tar.bz2",
            archive_bytes: bytes.len() as u64,
            archive_sha256: Box::leak(hex.into_boxed_str()),
            files: FILES,
        };
        (bytes, spec)
    }

    fn import(root: &Path, spec: &ModelSpec, bytes: &[u8]) -> Result<(), TranscribeError> {
        install(
            root,
            spec,
            Cursor::new(bytes),
            &AtomicBool::new(false),
            &mut |_| {},
        )
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|d| {
                d.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn the_pinned_archive_is_unpacked_with_only_the_needed_files() {
        let root = TempDir::new().unwrap();
        let (bytes, spec) = archive(&[]);
        assert!(!is_installed(root.path(), &spec));
        let mut seen = Vec::new();
        install(
            root.path(),
            &spec,
            Cursor::new(&bytes),
            &AtomicBool::new(false),
            &mut |n| seen.push(n),
        )
        .unwrap();
        assert!(is_installed(root.path(), &spec));
        assert_eq!(entries(root.path()), vec!["synthetic-model"]);
        let dir = model_dir(root.path(), &spec);
        assert_eq!(entries(&dir), vec!["model.onnx", "tokens.txt"]);
        assert_eq!(fs::read(dir.join("tokens.txt")).unwrap(), b"a 0\n\n");
        assert_eq!(seen.last(), Some(&spec.archive_bytes));
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress only grows");

        // Importing again replaces it; removing turns it off.
        import(root.path(), &spec, &bytes).unwrap();
        assert!(is_installed(root.path(), &spec));
        remove(root.path(), &spec).unwrap();
        assert!(!is_installed(root.path(), &spec));
        assert!(entries(root.path()).is_empty());
        remove(root.path(), &spec).unwrap();
    }

    #[test]
    fn any_other_file_is_refused_and_leaves_nothing() {
        let root = TempDir::new().unwrap();
        let (bytes, spec) = archive(&[]);
        let refuse = |bytes: &[u8], spec: &ModelSpec| {
            assert_eq!(
                import(root.path(), spec, bytes),
                Err(TranscribeError::ModelInvalid)
            );
            assert!(entries(root.path()).is_empty(), "nothing left behind");
        };
        // Wrong checksum, a flipped byte, truncated, a byte appended, empty, not bzip2.
        refuse(
            &bytes,
            &ModelSpec {
                archive_sha256: "00",
                ..spec
            },
        );
        let mut flipped = bytes.clone();
        let middle = flipped.len() / 2;
        flipped[middle] ^= 0x01;
        refuse(&flipped, &spec);
        refuse(&bytes[..bytes.len() - 10], &spec);
        let mut longer = bytes.clone();
        longer.push(0);
        refuse(&longer, &spec);
        refuse(b"", &spec);
        refuse(b"PK\x03\x04 a zip, not the model", &spec);

        // Files of the wrong size, missing or repeated, even with a matching checksum.
        let (odd, odd_spec) = archive(&[]);
        refuse(
            &odd,
            &ModelSpec {
                files: &[("model.onnx", 2999), ("tokens.txt", 5)],
                ..odd_spec
            },
        );
        refuse(
            &odd,
            &ModelSpec {
                files: &[("model.onnx", 3000), ("tokens.txt", 5), ("joiner.onnx", 1)],
                ..odd_spec
            },
        );
        let (twice, twice_spec) = archive(&[("synthetic-model/tokens.txt", b"a 0\n\n")]);
        refuse(&twice, &twice_spec);
    }

    #[test]
    fn paths_outside_the_model_folder_are_never_written() {
        let root = TempDir::new().unwrap();
        let (bytes, spec) = archive(&[
            ("model.onnx", &[1u8; 3000]),
            ("other/tokens.txt", b"x"),
            ("synthetic-model/sub/tokens.txt", b"x"),
        ]);
        // Same-named files in other folders are skipped, not unpacked.
        import(root.path(), &spec, &bytes).unwrap();
        assert_eq!(entries(root.path()), vec!["synthetic-model"]);
        assert_eq!(
            entries(&model_dir(root.path(), &spec)),
            vec!["model.onnx", "tokens.txt"]
        );
        assert_eq!(
            wanted(&spec, Path::new("synthetic-model/../tokens.txt")),
            None
        );
        assert_eq!(
            wanted(&spec, Path::new("/synthetic-model/tokens.txt")),
            None
        );
        assert_eq!(
            wanted(&spec, Path::new("./synthetic-model/tokens.txt")),
            Some(("tokens.txt", 5))
        );
    }

    #[test]
    fn a_cancelled_import_leaves_nothing() {
        let root = TempDir::new().unwrap();
        let (bytes, spec) = archive(&[]);
        let result = install(
            root.path(),
            &spec,
            Cursor::new(&bytes),
            &AtomicBool::new(true),
            &mut |_| {},
        );
        assert_eq!(result, Err(TranscribeError::Cancelled));
        assert!(entries(root.path()).is_empty());
    }

    #[test]
    fn a_damaged_model_on_disk_counts_as_missing() {
        let root = TempDir::new().unwrap();
        let (bytes, spec) = archive(&[]);
        import(root.path(), &spec, &bytes).unwrap();
        let tokens = model_dir(root.path(), &spec).join("tokens.txt");
        fs::write(&tokens, b"cut").unwrap();
        assert!(!is_installed(root.path(), &spec));
        fs::remove_file(&tokens).unwrap();
        assert!(!is_installed(root.path(), &spec));
    }

    /// Imports the real archive when `MOSSBOOK_MODEL_ARCHIVE` points to it and
    /// `MOSSBOOK_MODEL_ROOT` to the folder to import into (487 MB, not in CI).
    #[test]
    #[ignore = "needs the downloaded archive; set MOSSBOOK_MODEL_ARCHIVE and MOSSBOOK_MODEL_ROOT"]
    fn the_real_archive_imports() {
        let archive = std::env::var("MOSSBOOK_MODEL_ARCHIVE").expect("MOSSBOOK_MODEL_ARCHIVE");
        let root = std::env::var("MOSSBOOK_MODEL_ROOT").expect("MOSSBOOK_MODEL_ROOT");
        let file = File::open(archive).unwrap();
        let root = Path::new(&root);
        install(
            root,
            &PARAKEET_V3,
            file,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert!(is_installed(root, &PARAKEET_V3));
    }

    #[test]
    fn the_real_model_is_pinned() {
        assert_eq!(PARAKEET_V3.archive_sha256.len(), 64);
        assert!(
            PARAKEET_V3
                .url
                .ends_with(&format!("/{}.tar.bz2", PARAKEET_V3.name))
        );
        let total: u64 = PARAKEET_V3.files.iter().map(|(_, b)| b).sum();
        assert_eq!(total, 670_478_772);
    }
}
