use std::process::Command;
use tokio::fs;

pub async fn img_to_webp(img: &str, target: &String) {
    Command::new("magick")
        .arg("convert")
        .arg(img)
        .arg(("./data/".to_owned() + &target.to_owned() + &".webp".to_owned()).as_str())
        .status()
        .expect("failed to convert");

    fs::remove_file(img)
        .await
        .expect("failed to remove the tmp file");
}


pub fn webp_to_x(x: &str, target: &str, size: (i32, i32)) -> String {
    let tmp_file = format!("./cache/{target}@{}x{}.{x}", size.0, size.1);
    if size == (-1, -1) {
        std::process::Command::new("magick")
            .arg(format!("./data/{target}"))
            .arg(&tmp_file)
            .status()
            .expect("Failed to convert");
    } else {
        std::process::Command::new("magick")
            .arg(format!("./data/{target}"))
            .arg(&tmp_file)
            .arg("-resize")
            .arg(format!("{}x{}", size.0, size.1))
            .status()
            .expect("Failed to convert");
    }
    tmp_file
}

fn as_is(target: &str, size: (i32, i32)) -> String {
    if size == (-1, -1) {
        format!("./data/{}", target).to_string()
    } else {
        let tmp_file = format!("./cache/{}@{}x{}", target, size.0, size.1);
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("-resize")
            .arg(format!("{}x{}", size.0, size.1))
            .arg(&tmp_file)
            .status()
            .expect("a");
        tmp_file
    }
}

pub async fn convert_output(filetype: &str, target: &str, size: (i32, i32)) -> String {
    return match filetype.to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "svg" | "heic" => webp_to_x(&filetype, target, size),
        _ => as_is(target, size),
    };
}

async fn no_conversion(tmp_file: &str, target: &str) {
    fs::copy(tmp_file, target).await;
    fs::remove_file(tmp_file).await;
}

pub async fn convert_intake(filetype: &str, tmp_file: &str, target: &str) {
    match filetype.to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "svg" | "heic" => img_to_webp(&tmp_file, &target.to_string()).await,
        _ => no_conversion(&tmp_file, &target).await,
    };
}
