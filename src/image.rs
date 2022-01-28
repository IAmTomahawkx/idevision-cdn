use crate::imaging;
use actix_files::NamedFile;
use std::path;

#[derive(Debug, Clone)]
pub struct StoredItem {
    name: String,
    filetype: String,
}

fn extension_is_image(ext: &str) -> bool {
    match ext {
        "png" | "jpg" | "jpeg" | "svg" | "webp" => true,
        _ => false,
    }
}

fn stored_extension(ext: &str) -> &str {
    if extension_is_image(ext) {
        "webp"
    } else {
        ext
    }
}

impl StoredItem {
    fn new(name: &str) -> Option<Self> {
        let nm = path::Path::new(name);
        let stem = nm.extension()?.to_str()?.to_string();
        let nxm = nm.file_name()?.to_str()?.to_string();
        nxm.strip_suffix(&stem)?;

        Some(Self {
            name: nxm.to_string(),
            filetype: stem,
        })
    }

    fn exists(&self) -> bool {
        let formatted = format!("data/{:}.{:}", self.name, stored_extension(&self.filetype));
        let pth = path::Path::new(&formatted);
        pth.exists()
    }

    async fn to_named_file_with_size(&self, size: (i32, i32)) -> Result<NamedFile, std::io::Error> {
        if self.filetype.eq(stored_extension(&self.filetype.as_str())) && self.exists() {
            let formatted = format!("data/{:}.{:}", self.name, stored_extension(&self.filetype));
            NamedFile::open(formatted)
        }
        else {
            let pth = imaging::convert_output(self.filetype.as_str(), self.name.as_str(), size).await;
            NamedFile::open(pth)
        }
    }

    async fn to_named_file(&self) -> Result<NamedFile, std::io::Error> {
        if self.filetype.eq(stored_extension(&self.filetype.as_str())) && self.exists() {
            let formatted = format!("data/{:}.{:}", self.name, stored_extension(&self.filetype));
            NamedFile::open(formatted)
        }
        else {
            let pth = imaging::convert_output(self.filetype.as_str(), self.name.as_str(), (-1, -1)).await;
            NamedFile::open(pth)
        }
    }
}
