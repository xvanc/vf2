#![feature(exit_status_error)]

use std::{
    env,
    error::Error,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> Result<(), Box<dyn Error>> {
    preprocess()?;
    println!("cargo:rustc-link-arg-bin=chainload=--script=./linker.ld");
    Ok(())
}

fn preprocess() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR")
        .map(PathBuf::from)
        .expect("`OUT_DIR` variable not set");
    let cc = env::var("CC").unwrap_or_else(|_| "clang".into());

    let cc_is_clang = {
        let output = Command::new(&cc).arg("--version").output()?;
        output.status.exit_ok()?;
        output
            .stdout
            .split(|b| *b == b' ')
            .any(|slice| slice == b"clang")
    };

    let cflags: &[_] = match cc_is_clang {
        true => &["-target", "riscv64", "-march=rv64imac"],
        false => &["-march=rv64imac_zicsr_zifencei"],
    };

    walk_dir("src", |dirent| {
        let path = dirent.path();

        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("s"))
        {
            return Ok(());
        }

        let output = out_dir.join(&path);
        fs::create_dir_all(output.parent().unwrap())?;

        Command::new(&cc)
            .args(cflags)
            .args([
                "-mabi=lp64",
                "-E",
                "-xassembler-with-cpp",
                "-o",
            ])
            .arg(output)
            .arg(&path)
            .spawn()?
            .wait()?
            .exit_ok()?;

        println!("cargo:rerun-if-changed={}", path.display());

        Ok(())
    })?;

    Ok(())
}

fn walk_dir(
    path: impl AsRef<Path>,
    mut f: impl FnMut(DirEntry) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    fn _walk_dir(
        path: &Path,
        f: &mut impl FnMut(DirEntry) -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        for dirent in fs::read_dir(path)? {
            let dirent = dirent?;
            if dirent.file_type()?.is_dir() {
                _walk_dir(&dirent.path(), f)?;
            } else {
                f(dirent)?;
            }
        }
        Ok(())
    }
    _walk_dir(path.as_ref(), &mut f)
}
