use std::process::Command;
use tokio::fs;
use rand::random;

pub fn gen_tmp_name(filetype: &str) -> String {
    let mut tmp_name = "./temp/".to_string();
    let tmp_file: String = (0..10).map(|_| (0x20u8 + (random::<f32>() * 96.0) as u8) as char).collect();
    tmp_name.push_str(&tmp_file);
    tmp_name.push_str(&*format!(".{}", filetype));
    tmp_name
}

pub async fn img_to_webp(img: &str, target: &String) {
    Command::new("magick")
        .arg("convert")
        .arg(img)
        .arg(("./data/".to_owned() + &target.to_owned()+ &".webp".to_owned()).as_str())
        .status()
        .expect("failed to convert");

    fs::remove_file(img).await.expect("failed to remove the tmp file");
}

pub async fn webp_to_jpg(target: &str, size: (i32, i32)) -> String {
    let tmp_file = format!("./cache/{}@{}x{}.jpg", target, size.0, size.1);
    if size == (-1, -1) {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("jpg:".to_owned() + &tmp_file)
            .status()
            .expect("a");
    } else {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("-resize")
            .arg(format!("{}x{}", size.0, size.1))
            .arg("jpg:".to_owned() + &tmp_file)
            .status()
            .expect("a");
    }

    tmp_file
}

pub async fn webp_to_png(target: &str, size: (i32, i32)) -> String {
    let tmp_file = format!("./cache/{}@{}x{}.png", target, size.0, size.1);
    if size == (-1, -1) {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("png:".to_owned() + &tmp_file)
            .status()
            .expect("a");
    } else {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("-resize")
            .arg(format!("{}x{}", size.0, size.1))
            .arg("png:".to_owned() + &tmp_file)
            .status()
            .expect("a");
    }

    tmp_file
}

pub async fn webp_to_svg(target: &str, size: (i32, i32)) -> String {
    let tmp_file = format!("./cache/{}@{}x{}.svg", target, size.0, size.1);
    if size == (-1, -1) {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("svg:".to_owned() + &tmp_file)
            .status()
            .expect("a");
    } else {
        std::process::Command::new("magick")
            .arg("convert")
            .arg(format!("./data/{}", target))
            .arg("-resize")
            .arg(format!("{}x{}", size.0, size.1))
            .arg("svg:".to_owned() + &tmp_file)
            .status()
            .expect("a");
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
    return match filetype {
        "png" => webp_to_png(target, size).await,
        "jpg"|"jpeg" => webp_to_jpg(target, size).await,
        "svg" => webp_to_svg(target, size).await,
        _ => as_is(target, size)
    }
}

async fn no_conversion(tmp_file: &str, target: &str) {
    fs::copy(tmp_file, target).await;
    fs::remove_file(tmp_file).await;
}

pub async fn convert_intake(filetype: &str, tmp_file: &str, target: &str) {
    return match filetype {
        "png" | "jpg" | "jpeg" | "svg" => img_to_webp(&tmp_file, &target.to_string()).await,
        _ => no_conversion(&tmp_file, &target).await
    }
}