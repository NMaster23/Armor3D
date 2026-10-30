use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::PathBuf;
use rfd::AsyncFileDialog;
use std::time::SystemTime;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

pub fn write_3mf<W: Write + Seek>(writer: W, triangles: &[Triangle]) -> Result<(), Box<dyn Error>> {
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("[Content_Types].xml", options)?;
    let content_types = r#"<?xml version="1.0" encoding="UTF-8"?>
    <Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
        <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml" />
        <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml" />
    </Types>"#;
    zip.write_all(content_types.as_bytes())?;
    zip.start_file("_rels/.rels", options)?;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
    <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
        <Relationship Target="/3D/3dmodel.model" Id="rel10" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" />
    </Relationships>"#;
    zip.write_all(rels.as_bytes())?;
    zip.start_file("3D/3dmodel.model", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
        <model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
        <metadata name="Application">Armor3D</metadata>
        <resources>
            <object id="1" type="model">
                <mesh>
                <vertices>
    "#)?;
    for tri in triangles {
        let v_str = format!(
            "<vertex x=\"{:.4}\" y=\"{:.4}\" z=\"{:.4}\" />\n\
            <vertex x=\"{:.4}\" y=\"{:.4}\" z=\"{:.4}\" />\n\
            <vertex x=\"{:.4}\" y=\"{:.4}\" z=\"{:.4}\" />\n",
            tri.v0[0], tri.v0[1], tri.v0[2],
            tri.v1[0], tri.v1[1], tri.v1[2],
            tri.v2[0], tri.v2[1], tri.v2[2]
        );
        zip.write_all(v_str.as_bytes())?;
    }
    zip.write_all(br#"</vertices>
        <triangles>
    "#)?;
    for i in 0..triangles.len() {
        let v0 = i * 3;
        let v1 = i * 3 + 1;
        let v2 = i * 3 + 2;
        let tri_str = format!("<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\" />\n", v0, v1, v2);
        zip.write_all(tri_str.as_bytes())?;
    }
    zip.write_all(br#"        </triangles>
        </mesh>
        </object>
    </resources>
    <build>
        <item objectid="1" />
    </build>
    </model>"#)?;
    zip.finish()?;
    Ok(())
}

pub async fn write_file(triangles: &[Triangle], file_type: &str) -> Option<FileObject> {
    let path = AsyncFileDialog::new()
        .set_title("Save Output")
        .set_file_name(&format!("untitled_{:?}.obj", SystemTime::now()))
        .add_filter("Supported formats", ALLOWED_EXTENSIONS)
        .save_file()
        .await?;
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
    let mut writer = BufWriter::new(File::create(&final_path).ok()?);
    match final_ext.as_str() {
        "3mf" => {
            write_3mf(writer, triangles).ok()?;
        }
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