//! Thumbnails for the HTML pages. They are written next to the original file,
//! with a postfix in the name, the way Telegram Desktop does it.

use std::fs;
use std::path::Path;

use image::{ImageFormat, imageops::FilterType};

/// Images with a side this long or longer are not thumbnailed.
const MAX_IMAGE_SIZE: u32 = 10000;
const JPEG_QUALITY: u8 = 75;

pub const PHOTO_MAX: i32 = 520;
pub const PHOTO_MIN: i32 = 80;
pub const STICKER_MAX: i32 = 384;
pub const ENTRY_USERPIC_SIZE: i32 = 48;

pub type Size = (i32, i32);

/// `QSize::scaled` with `Qt::KeepAspectRatio`, integer arithmetic included.
fn scaled_keep_aspect(size: Size, bound: Size) -> Size {
    let (wd, ht) = (size.0 as i64, size.1 as i64);
    if wd == 0 || ht == 0 {
        return bound;
    }
    let rw = bound.1 as i64 * wd / ht;
    if rw <= bound.0 as i64 {
        (rw as i32, bound.1)
    } else {
        (bound.0, (bound.0 as i64 * ht / wd) as i32)
    }
}

/// The size a thumbnail gets: made smaller if it is too big (counting double
/// for the retina display when asked), rounded down to even numbers, and
/// dropped if it would be too small.
pub fn calculate_thumb_size(
    max_width: i32,
    max_height: i32,
    expand_for_retina: bool,
) -> impl Fn(Size) -> Option<Size> {
    move |large: Size| {
        let multiplier = if expand_for_retina { 2 } else { 1 };
        let small = if large.0 * multiplier > max_width || large.1 * multiplier > max_height {
            scaled_keep_aspect(large, (max_width, max_height))
        } else {
            large
        };
        let even = (small.0 & !1, small.1 & !1);
        if even.0 < PHOTO_MIN || even.1 < PHOTO_MIN {
            None
        } else {
            Some(even)
        }
    }
}

fn is_readable_image(path: &Path) -> bool {
    image::ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .map(|r| r.format().is_some())
        .unwrap_or(false)
}

/// Whether the file can be read as a picture the browser will show.
pub fn can_read_image(root: &Path, relative: &str) -> bool {
    if relative.is_empty() {
        return false;
    }
    let path = root.join(relative);
    is_readable_image(&path) && image::open(&path).is_ok()
}

/// `a/b.c.d` + `_thumb` becomes `a/b_thumb.c.d`.
fn thumb_name(large: &str, postfix: &str) -> String {
    let last_slash = large.rfind('/').map(|i| i + 1).unwrap_or(0);
    match large[last_slash..].find('.') {
        Some(dot) => {
            let at = last_slash + dot;
            format!("{}{}{}", &large[..at], postfix, &large[at..])
        }
        None => format!("{large}{postfix}"),
    }
}

/// `name (1).ext` until the path is free.
fn free_path(root: &Path, wanted: &str) -> String {
    let mut candidate = wanted.to_string();
    let mut n = 0;
    while root.join(&candidate).exists() {
        n += 1;
        let p = Path::new(wanted);
        let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("file");
        let ext = p.extension().and_then(|x| x.to_str());
        let dir = p.parent().and_then(|x| x.to_str()).unwrap_or("");
        let name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        candidate = if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        };
    }
    candidate
}

/// Writes a thumbnail of `large` and returns its relative path and size, or
/// `None` when the file is not a usable picture.
pub fn write_image_thumb(
    root: &Path,
    large: &str,
    convert: &dyn Fn(Size) -> Option<Size>,
    force_format: Option<ImageFormat>,
    postfix: &str,
) -> Option<(String, Size)> {
    if large.is_empty() {
        return None;
    }
    let path = root.join(large);
    let reader = image::ImageReader::open(&path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let (w, h) = reader.into_dimensions().ok()?;
    if w == 0 || h == 0 || w >= MAX_IMAGE_SIZE || h >= MAX_IMAGE_SIZE {
        return None;
    }
    let decoded = image::open(&path).ok()?;
    let final_size = convert((decoded.width() as i32, decoded.height() as i32))?;
    let resized = image::imageops::resize(
        &decoded.to_rgba8(),
        final_size.0 as u32,
        final_size.1 as u32,
        FilterType::Triangle,
    );
    let out_format = force_format.unwrap_or(format);
    let relative = free_path(root, &thumb_name(large, postfix));
    let target = root.join(&relative);
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).ok()?;
    }
    let file = fs::File::create(&target).ok()?;
    let mut out = std::io::BufWriter::new(file);
    let saved = match out_format {
        ImageFormat::Jpeg => {
            let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
            rgb.write_with_encoder(enc).is_ok()
        }
        other => image::DynamicImage::ImageRgba8(resized)
            .write_to(&mut out, other)
            .is_ok(),
    };
    if !saved {
        let _ = fs::remove_file(&target);
        return None;
    }
    Some((relative, final_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_name_goes_before_the_first_dot_of_the_file_name() {
        assert_eq!(
            thumb_name("chats/a.b/photo_1@x.jpg", "_thumb"),
            "chats/a.b/photo_1@x_thumb.jpg"
        );
        assert_eq!(thumb_name("noext", "_thumb"), "noext_thumb");
        assert_eq!(thumb_name("a/b.tar.gz", "_t"), "a/b_t.tar.gz");
    }

    #[test]
    fn small_images_are_not_thumbnailed_and_big_ones_shrink() {
        let f = calculate_thumb_size(PHOTO_MAX, PHOTO_MAX, false);
        assert_eq!(f((50, 50)), None);
        assert_eq!(f((200, 100)), Some((200, 100)));
        assert_eq!(f((1040, 520)), Some((520, 260)));
        let r = calculate_thumb_size(PHOTO_MAX, PHOTO_MAX, true);
        assert_eq!(r((640, 360)), Some((520, 292)));
    }
}
