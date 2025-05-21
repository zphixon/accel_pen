use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write as _,
};

fn main() {
    let _ = fs::create_dir("frontend/modules/src/bindings");
    let exports: Vec<_> = fs::read_dir("frontend/modules/src/bindings")
        .expect("read dir")
        .filter_map(Result::ok)
        .filter_map(|p| {
            p.path()
                .file_stem()
                .map(OsStr::to_str)
                .flatten()
                .map(str::to_owned)
        })
        .filter(|f| f != "index")
        .map(|f| format!("export * from \"./{}\"", f))
        .collect();

    let mut file = File::create("frontend/modules/src/bindings/index.ts").unwrap();
    file.write_all(exports.join("\n").as_bytes()).unwrap();
}
