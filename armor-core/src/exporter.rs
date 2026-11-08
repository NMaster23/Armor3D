use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::PathBuf;
use rfd::AsyncFileDialog;
use std::time::SystemTime;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const ALLOWED_EXTENSIONS: &[&str] = &["3mf", "stl", "obj"];

#[derive(Clone, Copy, Debug, PartialEq)]
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

pub fn parse_stl(bytes: &[u8]) -> Result<Vec<Triangle>, Box<dyn Error>> {
    if bytes.len() < 84 {
        return Err("STL File is too small".into())
    }
    let num_tris = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected_length = 84 + num_tris * 50;
    if bytes.len() >= expected_length {
        let mut tris = Vec::with_capacity(num_tris);
        for i in 0..num_tris {
            let offset = 84 + i * 50;
            let read_f32 = |start: usize| -> f32 {
                f32::from_le_bytes([bytes[start], bytes[start + 1], bytes[start + 2], bytes[start + 3]])
            };
            let norm = [read_f32(offset), read_f32(offset + 4), read_f32(offset + 8)];
            let v0 = [read_f32(offset + 12), read_f32(offset + 16), read_f32(offset + 20)];
            let v1 = [read_f32(offset + 24), read_f32(offset + 28), read_f32(offset + 32)];
            let v2 = [read_f32(offset + 36), read_f32(offset + 40), read_f32(offset + 44)];
            let n = if norm[0] == 0.0 && norm[1] == 0.0 && norm[2] == 0.0 {
                let d1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                let d2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
                let cross = [
                    d1[1] * d2[2] - d1[2] * d2[1],
                    d1[2] * d2[0] - d1[0] * d2[2],
                    d1[0] * d2[1] - d1[1] * d2[0],
                ];
                let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
                if len > 1e-6 {
                    [cross[0] / len, cross[1] / len, cross[2] / len]
                } else {
                    [0.0, 0.0, 1.0]
                }
            } else {
                norm
            };
            tris.push(Triangle {
                v0,
                v1,
                v2,
                normal: n,
            });
        }
        return Ok(tris);
    }
    let text = std::str::from_utf8(bytes)?;
    let mut tris = Vec::new();
    let mut current_n = [0.0, 0.0, 1.0];
    let mut current_verts: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("facet normal") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 5 {
                if let (Ok(nx), Ok(ny), Ok(nz)) = (parts[2].parse(), parts[3].parse(), parts[4].parse()) {
                    current_n = [nx, ny, nz];
                }
            }
            current_verts.clear();
        } else if trimmed.starts_with("vertex") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(x), Ok(y), Ok(z)) = (parts[1].parse(), parts[2].parse(), parts[3].parse()) {
                    current_verts.push([x, y, z]);
                }
            }
        } else if trimmed.starts_with("endfacet") {
            if current_verts.len() == 3 {
                tris.push(Triangle {
                    v0: current_verts[0],
                    v1: current_verts[1],
                    v2: current_verts[2],
                    normal: current_n,
                });
            }
            current_verts.clear();
        }
    }
    Ok(tris)
}

pub fn parse_obj(content: &str) -> Result<Vec<Triangle>, Box<dyn Error>> {
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<Triangle> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("v ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(x), Ok(y), Ok(z)) = (parts[1].parse(), parts[2].parse(), parts[3].parse()) {
                    verts.push([x, y, z]);
                }
            }
        } else if trimmed.starts_with("f ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let mut face_indices: Vec<usize> = Vec::new();
            for part in &parts[1..] {
                let v_str = part.split('/').next().unwrap_or("");
                if let Ok(idx) = v_str.parse::<i32>() {
                    let a_idx = if idx > 0 {
                        (idx as usize).saturating_sub(1)
                    } else {
                        (verts.len() as i32 + idx).max(0) as usize
                    };
                    if a_idx < verts.len() {
                        face_indices.push(a_idx);
                    }
                }
            }
            if face_indices.len() >= 3 {
                let v0 = verts[face_indices[0]];
                for k in 1..face_indices.len() - 1 {
                    let v1 = verts[face_indices[k]];
                    let v2 = verts[face_indices[k + 1]];
                    let d1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                    let d2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
                    let cross = [
                        d1[1] * d2[2] - d1[2] * d2[1],
                        d1[2] * d2[0] - d1[0] * d2[2],
                        d1[0] * d2[1] - d1[1] * d2[0],
                    ];
                    let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
                    let normal = if len > 1e-6 {
                        [cross[0] / len, cross[1] / len, cross[2] / len]
                    } else {
                        [0.0, 0.0, 1.0]
                    };
                    tris.push(Triangle {
                        v0,
                        v1,
                        v2,
                        normal,
                    });
                }
            }
        }
    }
    Ok(tris)
}

pub async fn read_mesh_file() -> Option<(Vec<Triangle>, String)> {
    let handle = AsyncFileDialog::new()
        .set_title("Import 3D Mesh")
        .add_filter("Supported Formats (*.stl, *.obj)", &["stl", "obj"])
        .pick_file()
        .await?;
    let path = handle.path();
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let bytes = std::fs::read(path).ok()?;
    let tris = match ext.as_str() {
        "stl" => parse_stl(&bytes).ok()?,
        "obj" => {
            let text = std::str::from_utf8(&bytes).ok()?;
            parse_obj(text).ok()?
        }
        _ => return None,
    };
    Some((tris, file_name))
}

pub fn read_mesh_from_path(path: &std::path::Path) -> Result<Vec<Triangle>, Box<dyn Error>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = std::fs::read(path)?;
    match ext.as_str() {
        "stl" => parse_stl(&bytes),
        "obj" => {
            let text = std::str::from_utf8(&bytes)?;
            parse_obj(text)
        }
        _ => Err("Unsupported format".into()),
    }
}