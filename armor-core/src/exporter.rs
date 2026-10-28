use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use rfd::AsyncFileDialog;
use std::time::SystemTime;

const ALLOWED_EXTENSIONS: &[&str] = &["3mf", "stl", "obj", "step", "glb/glTF"];

pub struct Triangle {
    pub v0: [f32; 3],
    pub v1: [f32; 3],
    pub v2: [f32; 3],
    pub normal: [f32; 3],
}

pub struct FileObject {
    pub full_path: PathBuf,
    pub file_name: String,
    pub file_stem: String,
    pub extension: String,
}


pub async fn write_file(triangles: &[Triangle], file_type: &str) -> Option<FileObject> {
    let path = AsyncFileDialog::new()
        .set_title("Save Output")
        .set_file_name(&format!("untitled_{:?}.obj", SystemTime::now()))
        .add_filter("Supported formats", ALLOWED_EXTENSIONS)
        .save_file()
        .await?;
    let mut writer = BufWriter::new(File::create(path.path()).ok()?);
    let mut header = [0u8; 80];
    let mut final_path = path.path().to_path_buf();
    let mut chosen_ext = final_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_ascii_lowercase());
    let final_ext = match chosen_ext {
        Some(ref ext) if ALLOWED_EXTENSIONS.contains(&ext.as_str()) => {
            ext.clone()
        }
        _ => {
            let default_ext = ALLOWED_EXTENSIONS[0];
            final_path.set_extension(default_ext);
            default_ext.to_string()
        }
    };
    let file_name = final_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let file_stem = final_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    match final_ext.as_str() {
        "stl" => {
            writer.write_all(&header).ok()?;
            writer.write_all(&(triangles.len() as u32).to_le_bytes()).ok()?;
            for tri in triangles {
                for c in &tri.normal { writer.write_all(&c.to_le_bytes()).ok()?; }
                for c in &tri.v0 { writer.write_all(&c.to_le_bytes()).ok()?; }
                for c in &tri.v1 { writer.write_all(&c.to_le_bytes()).ok()?; }
                for c in &tri.v2 { writer.write_all(&c.to_le_bytes()).ok()?; }
                writer.write_all(&[0u8, 0u8]).ok()?;
            }
        }
        "obj" => {
            writeln!(writer, "Armor3D Obj Export");
            let mut v_idx = 1;
            for tri in triangles {
                writeln!(writer, "v {} {} {}", tri.v0[0], tri.v0[1], tri.v0[2]).ok();
                writeln!(writer, "v {} {} {}", tri.v1[0], tri.v1[1], tri.v1[2]).ok();
                writeln!(writer, "v {} {} {}", tri.v2[0], tri.v2[1], tri.v2[2]).ok();
                writeln!(writer, "f {} {} {}", v_idx, v_idx + 1, v_idx + 2).ok();
                v_idx += 3;
            }
            writer.flush().ok();
        }
        _ => {}
    }
    Some(FileObject {
        full_path: final_path,
        file_name,
        file_stem,
        extension: final_ext,
    })
}