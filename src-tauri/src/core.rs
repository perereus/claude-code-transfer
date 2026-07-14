//! Lógica pura de escaneo, exportación e importación. Sin dependencias de Tauri
//! para poder testearla con `cargo test`.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const MANIFEST_VERSION: u32 = 1;

// ---------- tipos ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub size: u64,
    pub modified_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub slug: String,
    pub real_path: String,
    pub name: String,
    pub folder_exists: bool,
    pub folder_size: u64,
    pub chat_size: u64,
    pub last_modified_ms: u64,
    pub sessions: Vec<SessionInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub exported_at: String,
    pub source_home: String,
    pub projects: Vec<ManifestProject>,
    /// rutas relativas a ~/.claude de los archivos de config incluidos
    #[serde(default)]
    pub config: Vec<String>,
}

/// Lista blanca de configuración global exportable (rutas relativas a ~/.claude).
/// NUNCA se incluye `.credentials.json` (tokens de auth) ni los `*-cache.json`
/// (estado por-máquina, se regeneran solos).
pub const CONFIG_FILES: &[&str] = &[
    "settings.json",
    "settings.local.json",
    "keybindings.json",
    "CLAUDE.md",
    "plugins/installed_plugins.json",
    "plugins/known_marketplaces.json",
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub rel_path: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreview {
    pub rel_path: String,
    pub size: u64,
    pub exists_locally: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProject {
    pub slug: String,
    pub real_path: String,
    pub sessions: Vec<SessionInfo>,
    pub includes_files: bool,
    pub exclusions: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelection {
    pub slug: String,
    pub real_path: String,
    pub session_ids: Vec<String>,
    pub include_files: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePreview {
    pub source_home: String,
    pub exported_at: String,
    pub projects: Vec<PreviewProject>,
    pub config: Vec<ConfigPreview>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProject {
    pub slug: String,
    pub source_path: String,
    pub suggested_target_path: String,
    pub sessions: Vec<PreviewSession>,
    pub includes_files: bool,
    pub file_count: u64,
    pub files_size: u64,
    pub target_folder_exists: bool,
    pub existing_session_count: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSession {
    pub id: String,
    pub title: String,
    pub exists_locally: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportResolution {
    pub slug: String,
    pub import_chats: bool,
    /// "merge" = saltar sesiones que ya existen, "overwrite" = machacarlas
    pub session_mode: String,
    pub import_files: bool,
    /// true = solo escribir archivos más nuevos que los locales
    pub only_newer: bool,
    pub target_path: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResolution {
    /// rutas relativas a importar (subconjunto de las del archivo)
    pub rel_paths: Vec<String>,
    /// true = hacer copia `.bak-<ts>` del local antes de sobrescribir
    pub backup_existing: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub sessions_imported: u64,
    pub sessions_skipped: u64,
    pub files_written: u64,
    pub files_skipped: u64,
    pub history_lines_added: u64,
    pub config_written: u64,
    pub projects: Vec<String>,
}

// ---------- rutas y slugs ----------

/// `C:\Users\Pere\radar` -> `C--Users-Pere-radar` (regla de Claude Code:
/// todo carácter no alfanumérico pasa a `-`)
pub fn slug_for_path(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Variantes (patrón, reemplazo) de una ruta: niveles de escapado JSON
/// (JSON anidado en JSON multiplica los backslashes: 1, 2, 4, … 2^5) y forma
/// con barras `/`.
// ponytail: reemplazo sensible a mayúsculas; si otro PC escribe c:\users habrá que hacerlo case-insensitive
fn path_variants(from: &str, to: &str) -> Vec<(String, String)> {
    if from == to || from.is_empty() {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    for level in 0..=5u32 {
        let bs = "\\".repeat(2usize.pow(level));
        pairs.push((from.replace('\\', &bs), to.replace('\\', &bs)));
    }
    pairs.push((from.replace('\\', "/"), to.replace('\\', "/")));
    pairs
}

/// Reemplazo multi-patrón en una sola pasada: lo ya sustituido no se vuelve a
/// escanear (evita dobles reemplazos cuando un `to` contiene otro `from`).
/// Empate en la misma posición: gana el patrón más largo.
fn multi_replace(content: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len());
    let mut pos = 0usize;
    let mut next: Vec<Option<usize>> =
        pairs.iter().map(|(p, _)| content.find(p.as_str())).collect();
    loop {
        let mut best: Option<(usize, usize)> = None; // (posición, índice de patrón)
        for (i, n) in next.iter().enumerate() {
            if let Some(p) = *n {
                let better = match best {
                    None => true,
                    Some((bp, bi)) => p < bp || (p == bp && pairs[i].0.len() > pairs[bi].0.len()),
                };
                if better {
                    best = Some((p, i));
                }
            }
        }
        let Some((mpos, mi)) = best else { break };
        out.push_str(&content[pos..mpos]);
        out.push_str(&pairs[mi].1);
        pos = mpos + pairs[mi].0.len();
        for (i, n) in next.iter_mut().enumerate() {
            if n.is_some_and(|p| p < pos) {
                *n = content[pos..].find(pairs[i].0.as_str()).map(|o| o + pos);
            }
        }
    }
    out.push_str(&content[pos..]);
    out
}

/// Reemplaza `from` por `to` en todas las formas en que aparecen rutas dentro
/// de los jsonl: JSON-escapada (a cualquier nivel), normal y con barras `/`.
pub fn remap_content(content: &str, from: &str, to: &str) -> String {
    multi_replace(content, &path_variants(from, to))
}

/// Remapea ruta de proyecto y home en una sola pasada (para rutas tipo
/// ~/.claude/... que no cuelgan del proyecto). Si una ruta encaja con ambos,
/// gana la del proyecto por ser más larga.
pub fn remap_all(content: &str, project_from: &str, project_to: &str, home_from: &str, home_to: &str) -> String {
    let mut pairs = path_variants(project_from, project_to);
    pairs.extend(path_variants(home_from, home_to));
    multi_replace(content, &pairs)
}

/// Ruta destino sugerida: si la ruta origen cuelga del home origen, la misma
/// ruta relativa bajo el home local; si no, la ruta tal cual.
pub fn suggest_target_path(source_path: &str, source_home: &str, target_home: &str) -> String {
    let fwd_src = source_path.replace('/', "\\");
    let fwd_home = source_home.replace('/', "\\");
    if let Some(rest) = fwd_src.strip_prefix(&fwd_home) {
        format!("{}{}", target_home.trim_end_matches('\\'), rest)
    } else {
        source_path.to_string()
    }
}

// ---------- escaneo ----------

fn modified_ms(md: &fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Primer prompt del usuario en un jsonl de sesión (título para la UI).
fn session_title_from_jsonl(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    for line in BufReader::new(file).lines().take(200).flatten() {
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("user") {
            continue;
        }
        if v.get("isMeta").and_then(|m| m.as_bool()) == Some(true) {
            continue;
        }
        let content = v.get("message").and_then(|m| m.get("content"))?;
        let text = match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(items) => items
                .iter()
                .find_map(|i| {
                    (i.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .then(|| i.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        let text = text.trim();
        if text.is_empty() || text.starts_with('<') || text.starts_with("Caveat") {
            continue;
        }
        let title: String = text.chars().take(100).collect();
        return Some(title);
    }
    None
}

/// sessionId -> primer prompt, desde history.jsonl (títulos más limpios).
fn history_titles(claude_dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(file) = fs::File::open(claude_dir.join("history.jsonl")) else {
        return map;
    };
    for line in BufReader::new(file).lines().flatten() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let (Some(sid), Some(display)) = (
            v.get("sessionId").and_then(|s| s.as_str()),
            v.get("display").and_then(|d| d.as_str()),
        ) else {
            continue;
        };
        map.entry(sid.to_string())
            .or_insert_with(|| display.chars().take(100).collect());
    }
    map
}

/// `cwd` de la primera línea que lo tenga (fuente fiable de la ruta real).
fn project_cwd_from_sessions(dir: &Path) -> Option<String> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = fs::File::open(&p) else { continue };
        for line in BufReader::new(file).lines().take(50).flatten() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                    return Some(cwd.to_string());
                }
            }
        }
    }
    None
}

/// Tamaño de una carpeta respetando las exclusiones elegidas por el usuario
/// (las carpetas excluidas se podan sin descender, para que node_modules/.git
/// no cuesten minutos).
pub fn dir_size(path: &Path, exclusions: &[String]) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && exclusions
                    .iter()
                    .any(|x| e.file_name().to_string_lossy().eq_ignore_ascii_case(x)))
        })
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Recalcula solo los tamaños de carpeta (para refrescar la UI al cambiar exclusiones).
pub fn folder_sizes(paths: &[String], exclusions: &[String]) -> Vec<u64> {
    paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            if path.is_dir() { dir_size(path, exclusions) } else { 0 }
        })
        .collect()
}

/// Archivos de la lista blanca de config que existen en este ~/.claude.
pub fn list_config(claude_dir: &Path) -> Vec<ConfigFile> {
    CONFIG_FILES
        .iter()
        .filter_map(|rel| {
            let md = fs::metadata(claude_dir.join(rel)).ok()?;
            md.is_file().then(|| ConfigFile { rel_path: rel.to_string(), size: md.len() })
        })
        .collect()
}

pub fn list_projects(claude_dir: &Path, exclusions: &[String]) -> io::Result<Vec<ProjectInfo>> {
    let titles = history_titles(claude_dir);
    let projects_dir = claude_dir.join("projects");
    let mut result = Vec::new();

    for entry in fs::read_dir(&projects_dir)?.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();

        let mut sessions = Vec::new();
        let mut chat_size = 0u64;
        let mut last_modified = 0u64;
        for f in fs::read_dir(&dir)?.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let id = p.file_stem().unwrap().to_string_lossy().to_string();
            let md = f.metadata()?;
            let mtime = modified_ms(&md);
            chat_size += md.len();
            last_modified = last_modified.max(mtime);
            let title = titles
                .get(&id)
                .cloned()
                .or_else(|| session_title_from_jsonl(&p))
                .unwrap_or_else(|| "(sin título)".to_string());
            sessions.push(SessionInfo { id, title, size: md.len(), modified_ms: mtime });
        }
        if sessions.is_empty() {
            continue;
        }
        sessions.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));

        let real_path = project_cwd_from_sessions(&dir).unwrap_or_else(|| slug.replace('-', "\\"));
        let folder = Path::new(&real_path);
        let folder_exists = folder.is_dir();
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| slug.clone());

        result.push(ProjectInfo {
            slug,
            name,
            folder_size: if folder_exists { dir_size(folder, exclusions) } else { 0 },
            folder_exists,
            real_path,
            chat_size,
            last_modified_ms: last_modified,
            sessions,
        });
    }
    result.sort_by(|a, b| b.last_modified_ms.cmp(&a.last_modified_ms));
    Ok(result)
}

// ---------- exportación ----------

fn is_excluded(rel: &Path, exclusions: &[String]) -> bool {
    rel.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        exclusions.iter().any(|x| x.eq_ignore_ascii_case(&name))
    })
}

pub fn export_projects<F: FnMut(&str, u64, u64)>(
    claude_dir: &Path,
    home: &str,
    selections: &[ExportSelection],
    exclusions: &[String],
    config_files: &[String],
    dest_zip: &Path,
    mut progress: F,
) -> io::Result<()> {
    let file = fs::File::create(dest_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);

    let mut manifest_projects = Vec::new();

    // pre-pase: lista concreta de trabajo para dar progreso en bytes reales.
    // Un solo walk por proyecto; se reutiliza al escribir el zip.
    // ponytail: sin progreso intra-archivo; si un único archivo gigante lo pide, copiar en chunks
    let mut project_files: Vec<Vec<(PathBuf, String, u64)>> = Vec::new();
    let mut total_bytes = 0u64;
    for sel in selections {
        let chat_dir = claude_dir.join("projects").join(&sel.slug);
        for id in &sel.session_ids {
            total_bytes += fs::metadata(chat_dir.join(format!("{id}.jsonl")))?.len();
        }
        let mut files = Vec::new();
        if sel.include_files {
            let root = Path::new(&sel.real_path);
            for entry in WalkDir::new(root).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let rel = entry.path().strip_prefix(root).unwrap();
                if is_excluded(rel, exclusions) {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let rel_fwd = rel.to_string_lossy().replace('\\', "/");
                total_bytes += size;
                files.push((entry.path().to_path_buf(), rel_fwd, size));
            }
        }
        project_files.push(files);
    }
    for rel in config_files {
        if CONFIG_FILES.contains(&rel.as_str()) {
            total_bytes += fs::metadata(claude_dir.join(rel)).map(|m| m.len()).unwrap_or(0);
        }
    }
    let mut done_bytes = 0u64;

    for (i, sel) in selections.iter().enumerate() {
        progress(&format!("Exporting {}", sel.real_path), done_bytes, total_bytes);
        let chat_dir = claude_dir.join("projects").join(&sel.slug);
        let mut sessions_meta = Vec::new();

        for id in &sel.session_ids {
            let src = chat_dir.join(format!("{id}.jsonl"));
            let md = fs::metadata(&src)?;
            zip.start_file(format!("chats/{}/{}.jsonl", sel.slug, id), opts)
                .map_err(io::Error::other)?;
            io::copy(&mut fs::File::open(&src)?, &mut zip)?;
            done_bytes += md.len();
            progress(&format!("Exporting {}", sel.real_path), done_bytes, total_bytes);
            sessions_meta.push(SessionInfo {
                id: id.clone(),
                title: session_title_from_jsonl(&src).unwrap_or_default(),
                size: md.len(),
                modified_ms: modified_ms(&md),
            });
        }

        // history.jsonl filtrado por proyecto
        if let Ok(hist) = fs::File::open(claude_dir.join("history.jsonl")) {
            let needle = format!("\"project\":{}", serde_json::to_string(&sel.real_path)?);
            let lines: Vec<String> = BufReader::new(hist)
                .lines()
                .flatten()
                .filter(|l| l.contains(&needle))
                .collect();
            if !lines.is_empty() {
                zip.start_file(format!("history/{}.jsonl", sel.slug), opts)
                    .map_err(io::Error::other)?;
                zip.write_all(lines.join("\n").as_bytes())?;
                zip.write_all(b"\n")?;
            }
        }

        if sel.include_files {
            for (path, rel_fwd, size) in &project_files[i] {
                let mtime = fs::metadata(path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| zip::DateTime::try_from(time::OffsetDateTime::from(t)).ok());
                let mut fopts = opts;
                if let Some(t) = mtime {
                    fopts = fopts.last_modified_time(t);
                }
                zip.start_file(format!("files/{}/{}", sel.slug, rel_fwd), fopts)
                    .map_err(io::Error::other)?;
                io::copy(&mut fs::File::open(path)?, &mut zip)?;
                done_bytes += size;
                progress(&format!("Exporting {}", sel.real_path), done_bytes, total_bytes);
            }
        }

        manifest_projects.push(ManifestProject {
            slug: sel.slug.clone(),
            real_path: sel.real_path.clone(),
            sessions: sessions_meta,
            includes_files: sel.include_files,
            exclusions: exclusions.to_vec(),
        });
    }

    // configuración global (solo lista blanca; nunca credentials ni caches)
    let mut config_included = Vec::new();
    for rel in config_files {
        if !CONFIG_FILES.contains(&rel.as_str()) {
            continue; // defensa: ignora cualquier ruta fuera de la lista blanca
        }
        let src = claude_dir.join(rel);
        if !src.is_file() {
            continue;
        }
        zip.start_file(format!("config/{rel}"), opts).map_err(io::Error::other)?;
        io::copy(&mut fs::File::open(&src)?, &mut zip)?;
        done_bytes += fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        progress("Exporting configuration", done_bytes, total_bytes);
        config_included.push(rel.clone());
    }

    let manifest = Manifest {
        version: MANIFEST_VERSION,
        exported_at: chrono_now(),
        source_home: home.to_string(),
        projects: manifest_projects,
        config: config_included,
    };
    zip.start_file("manifest.json", opts).map_err(io::Error::other)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.finish().map_err(io::Error::other)?;
    progress("Done", total_bytes, total_bytes);
    Ok(())
}

// ponytail: fecha ISO sin crate chrono — con SystemTime basta para el manifest
fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

// ---------- inspección ----------

fn read_manifest(zip_path: &Path) -> io::Result<Manifest> {
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(io::Error::other)?;
    let mut entry = archive.by_name("manifest.json").map_err(io::Error::other)?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(serde_json::from_str(&buf)?)
}

pub fn inspect_archive(zip_path: &Path, claude_dir: &Path, target_home: &str) -> io::Result<ArchivePreview> {
    let manifest = read_manifest(zip_path)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(io::Error::other)?;

    // tamaño y nº de archivos por proyecto
    let mut file_stats: HashMap<String, (u64, u64)> = HashMap::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(io::Error::other)?;
        let name = entry.name().to_string();
        if let Some(rest) = name.strip_prefix("files/") {
            if let Some(slug) = rest.split('/').next() {
                let s = file_stats.entry(slug.to_string()).or_default();
                s.0 += 1;
                s.1 += entry.size();
            }
        }
    }

    let mut projects = Vec::new();
    for p in &manifest.projects {
        let target_path = suggest_target_path(&p.real_path, &manifest.source_home, target_home);
        let local_chat_dir = claude_dir.join("projects").join(slug_for_path(&target_path));
        let sessions: Vec<PreviewSession> = p
            .sessions
            .iter()
            .map(|s| PreviewSession {
                exists_locally: local_chat_dir.join(format!("{}.jsonl", s.id)).exists(),
                id: s.id.clone(),
                title: s.title.clone(),
            })
            .collect();
        let existing = sessions.iter().filter(|s| s.exists_locally).count() as u64;
        let (file_count, files_size) = file_stats.get(&p.slug).copied().unwrap_or((0, 0));
        projects.push(PreviewProject {
            slug: p.slug.clone(),
            source_path: p.real_path.clone(),
            target_folder_exists: Path::new(&target_path).is_dir(),
            suggested_target_path: target_path,
            sessions,
            includes_files: p.includes_files,
            file_count,
            files_size,
            existing_session_count: existing,
        });
    }

    let config: Vec<ConfigPreview> = manifest
        .config
        .iter()
        .filter_map(|rel| {
            let size = archive.by_name(&format!("config/{rel}")).ok()?.size();
            Some(ConfigPreview {
                exists_locally: claude_dir.join(rel).exists(),
                rel_path: rel.clone(),
                size,
            })
        })
        .collect();

    Ok(ArchivePreview {
        source_home: manifest.source_home,
        exported_at: manifest.exported_at,
        projects,
        config,
    })
}

// ---------- importación ----------

pub fn import_projects<F: FnMut(&str, u64, u64)>(
    zip_path: &Path,
    claude_dir: &Path,
    target_home: &str,
    resolutions: &[ImportResolution],
    config: &ConfigResolution,
    mut progress: F,
) -> io::Result<ImportSummary> {
    let manifest = read_manifest(zip_path)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(io::Error::other)?;
    let mut summary = ImportSummary::default();

    // sessionIds ya presentes en history.jsonl local (para no duplicar)
    let history_path = claude_dir.join("history.jsonl");
    let existing_history: HashSet<String> = fs::File::open(&history_path)
        .map(|f| {
            BufReader::new(f)
                .lines()
                .flatten()
                .filter_map(|l| {
                    serde_json::from_str::<serde_json::Value>(&l)
                        .ok()?
                        .get("sessionId")?
                        .as_str()
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    // pre-pase: bytes totales a procesar (chats + archivos + config) para
    // dar progreso real en bytes; los saltados también cuentan como avance
    let mut total_bytes = 0u64;
    for res in resolutions {
        let Some(mp) = manifest.projects.iter().find(|p| p.slug == res.slug) else {
            continue;
        };
        if res.import_chats {
            total_bytes += mp.sessions.iter().map(|s| s.size).sum::<u64>();
        }
        if res.import_files && mp.includes_files {
            let prefix = format!("files/{}/", mp.slug);
            for i in 0..archive.len() {
                let entry = archive.by_index(i).map_err(io::Error::other)?;
                if entry.name().starts_with(&prefix) {
                    total_bytes += entry.size();
                }
            }
        }
    }
    for rel in &config.rel_paths {
        if CONFIG_FILES.contains(&rel.as_str()) {
            if let Ok(entry) = archive.by_name(&format!("config/{rel}")) {
                total_bytes += entry.size();
            }
        }
    }
    let mut done_bytes = 0u64;

    for res in resolutions.iter() {
        let Some(mp) = manifest.projects.iter().find(|p| p.slug == res.slug) else {
            continue;
        };
        progress(&format!("Importing {}", res.target_path), done_bytes, total_bytes);

        let remap = |content: &str| {
            remap_all(content, &mp.real_path, &res.target_path, &manifest.source_home, target_home)
        };
        let new_slug = slug_for_path(&res.target_path);

        if res.import_chats {
            let chat_dir = claude_dir.join("projects").join(&new_slug);
            fs::create_dir_all(&chat_dir)?;
            for s in &mp.sessions {
                let dest = chat_dir.join(format!("{}.jsonl", s.id));
                if dest.exists() && res.session_mode != "overwrite" {
                    summary.sessions_skipped += 1;
                    done_bytes += s.size;
                    progress(&format!("Importing {}", res.target_path), done_bytes, total_bytes);
                    continue;
                }
                let mut entry = archive
                    .by_name(&format!("chats/{}/{}.jsonl", mp.slug, s.id))
                    .map_err(io::Error::other)?;
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                fs::write(&dest, remap(&content))?;
                summary.sessions_imported += 1;
                done_bytes += s.size;
                progress(&format!("Importing {}", res.target_path), done_bytes, total_bytes);
            }

            // history remapeado, sin duplicar sessionId
            if let Ok(mut entry) = archive.by_name(&format!("history/{}.jsonl", mp.slug)) {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                let new_lines: Vec<String> = remap(&content)
                    .lines()
                    .filter(|l| {
                        serde_json::from_str::<serde_json::Value>(l)
                            .ok()
                            .and_then(|v| v.get("sessionId").and_then(|s| s.as_str()).map(String::from))
                            .map(|sid| !existing_history.contains(&sid))
                            .unwrap_or(false)
                    })
                    .map(String::from)
                    .collect();
                if !new_lines.is_empty() {
                    let mut f = fs::OpenOptions::new().create(true).append(true).open(&history_path)?;
                    f.write_all(new_lines.join("\n").as_bytes())?;
                    f.write_all(b"\n")?;
                    summary.history_lines_added += new_lines.len() as u64;
                }
            }
        }

        if res.import_files && mp.includes_files {
            let prefix = format!("files/{}/", mp.slug);
            let root = PathBuf::from(&res.target_path);
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(io::Error::other)?;
                let Some(rel) = entry.name().strip_prefix(&prefix).map(String::from) else {
                    continue;
                };
                if rel.is_empty() || rel.contains("..") {
                    continue;
                }
                let entry_size = entry.size();
                let dest = root.join(rel.replace('/', "\\"));
                if res.only_newer && dest.exists() {
                    let local_mtime = fs::metadata(&dest)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| time::OffsetDateTime::from(t).unix_timestamp())
                        .unwrap_or(0);
                    let zip_mtime = entry
                        .last_modified()
                        .and_then(|d| time::OffsetDateTime::try_from(d).ok())
                        .map(|t| t.unix_timestamp())
                        .unwrap_or(i64::MAX);
                    if zip_mtime <= local_mtime {
                        summary.files_skipped += 1;
                        done_bytes += entry_size;
                        progress(&format!("Importing {}", res.target_path), done_bytes, total_bytes);
                        continue;
                    }
                }
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                io::copy(&mut entry, &mut fs::File::create(&dest)?)?;
                summary.files_written += 1;
                done_bytes += entry_size;
                progress(&format!("Importing {}", res.target_path), done_bytes, total_bytes);
            }
        }

        summary.projects.push(res.target_path.clone());
    }

    // configuración global: remapea el home (los settings llevan rutas de hooks)
    // y hace copia .bak del local antes de sobrescribir si se pide
    for rel in &config.rel_paths {
        if !CONFIG_FILES.contains(&rel.as_str()) {
            continue; // defensa contra rutas fuera de la lista blanca
        }
        let Ok(mut entry) = archive.by_name(&format!("config/{rel}")) else {
            continue;
        };
        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        let remapped = remap_content(&content, &manifest.source_home, target_home);
        let dest = claude_dir.join(rel);
        if dest.exists() && config.backup_existing {
            let bak = dest.with_extension(format!(
                "{}.bak-{}",
                dest.extension().and_then(|e| e.to_str()).unwrap_or(""),
                chrono_now().trim_start_matches("unix:")
            ));
            fs::copy(&dest, &bak)?;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, remapped)?;
        summary.config_written += 1;
        done_bytes += content.len() as u64;
        progress("Importing configuration", done_bytes, total_bytes);
    }

    progress("Done", total_bytes, total_bytes);
    Ok(summary)
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_claude_code_rule() {
        assert_eq!(slug_for_path("C:\\Users\\Pere\\radar"), "C--Users-Pere-radar");
        assert_eq!(
            slug_for_path("C:\\Users\\Pere\\CLAUDE CODE\\SCREENSHOT OCR"),
            "C--Users-Pere-CLAUDE-CODE-SCREENSHOT-OCR"
        );
    }

    #[test]
    fn remap_rewrites_all_path_forms() {
        let content = r#"{"cwd":"C:\\Users\\Pere\\radar","x":"C:/Users/Pere/radar/a.txt","y":"C:\\Users\\Pere\\radar\\b"}"#;
        let out = remap_content(content, "C:\\Users\\Pere\\radar", "C:\\Users\\Ana\\radar");
        assert_eq!(
            out,
            r#"{"cwd":"C:\\Users\\Ana\\radar","x":"C:/Users/Ana/radar/a.txt","y":"C:\\Users\\Ana\\radar\\b"}"#
        );
    }

    #[test]
    fn remap_rewrites_double_escaped_paths() {
        // JSON embebido como string dentro de otro JSON (p. ej. config de hooks)
        let content = r#"{"cmd":"pwsh C:\\\\Users\\\\Pere\\\\.claude\\\\x.ps1"}"#;
        let out = remap_content(content, "C:\\Users\\Pere", "C:\\Users\\Ana");
        assert_eq!(out, r#"{"cmd":"pwsh C:\\\\Users\\\\Ana\\\\.claude\\\\x.ps1"}"#);
    }

    #[test]
    fn remap_same_path_is_noop() {
        let content = r#"{"cwd":"C:\\Users\\Pere\\radar"}"#;
        assert_eq!(remap_content(content, "C:\\Users\\Pere", "C:\\Users\\Pere"), content);
    }

    #[test]
    fn remap_no_double_replace_when_target_contains_source() {
        // destino que contiene el origen como prefijo: no debe re-remapear
        let content = r#"{"cwd":"C:\\Users\\Pere\\radar"}"#;
        let out = remap_all(
            content,
            "C:\\Users\\Pere\\radar",
            "C:\\Users\\Pere\\Temp\\otro\\radar",
            "C:\\Users\\Pere",
            "C:\\Users\\Pere\\Temp\\otro",
        );
        assert_eq!(out, r#"{"cwd":"C:\\Users\\Pere\\Temp\\otro\\radar"}"#);
    }

    #[test]
    fn remap_all_project_then_home() {
        let content = r#"{"cwd":"C:\\Users\\Pere\\radar","claude":"C:\\Users\\Pere\\.claude\\x"}"#;
        let out = remap_all(
            content,
            "C:\\Users\\Pere\\radar",
            "D:\\proyectos\\radar",
            "C:\\Users\\Pere",
            "C:\\Users\\Ana",
        );
        assert_eq!(
            out,
            r#"{"cwd":"D:\\proyectos\\radar","claude":"C:\\Users\\Ana\\.claude\\x"}"#
        );
    }

    #[test]
    fn suggested_target_swaps_home() {
        assert_eq!(
            suggest_target_path("C:\\Users\\Pere\\radar", "C:\\Users\\Pere", "C:\\Users\\Ana"),
            "C:\\Users\\Ana\\radar"
        );
        // ruta fuera del home: se conserva
        assert_eq!(
            suggest_target_path("D:\\code\\app", "C:\\Users\\Pere", "C:\\Users\\Ana"),
            "D:\\code\\app"
        );
    }

    #[test]
    fn export_then_import_roundtrip_with_remap() {
        let tmp = std::env::temp_dir().join(format!("cctx-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);

        // PC origen simulado
        let src_home = tmp.join("src-home");
        let src_claude = src_home.join(".claude");
        let project_path = src_home.join("miproyecto");
        let slug = slug_for_path(project_path.to_str().unwrap());
        let chat_dir = src_claude.join("projects").join(&slug);
        fs::create_dir_all(&chat_dir).unwrap();
        fs::create_dir_all(project_path.join("node_modules")).unwrap();
        fs::write(project_path.join("main.py"), "print('hola')").unwrap();
        fs::write(project_path.join("node_modules").join("x.js"), "junk").unwrap();

        let esc = project_path.to_str().unwrap().replace('\\', "\\\\");
        fs::write(
            chat_dir.join("abc-123.jsonl"),
            format!("{{\"type\":\"user\",\"cwd\":\"{esc}\",\"message\":{{\"role\":\"user\",\"content\":\"hola claude\"}}}}\n"),
        )
        .unwrap();
        fs::write(
            src_claude.join("history.jsonl"),
            format!("{{\"display\":\"hola claude\",\"project\":\"{esc}\",\"sessionId\":\"abc-123\"}}\n"),
        )
        .unwrap();
        // settings global con una ruta de hook que apunta al home origen
        let home_esc = src_home.to_str().unwrap().replace('\\', "\\\\");
        fs::write(
            src_claude.join("settings.json"),
            format!("{{\"hook\":\"{home_esc}\\\\.claude\\\\x.ps1\"}}"),
        )
        .unwrap();
        // credentials NO debe viajar aunque se pida
        fs::write(src_claude.join(".credentials.json"), "{\"token\":\"secreto\"}").unwrap();

        // exportar
        let zip_path = tmp.join("export.cctx");
        let sel = ExportSelection {
            slug: slug.clone(),
            real_path: project_path.to_str().unwrap().to_string(),
            session_ids: vec!["abc-123".into()],
            include_files: true,
        };
        let cfg = list_config(&src_claude);
        assert!(cfg.iter().any(|c| c.rel_path == "settings.json"));
        assert!(!cfg.iter().any(|c| c.rel_path == ".credentials.json"), "credentials en lista blanca");
        let mut prog: Vec<(u64, u64)> = Vec::new();
        export_projects(
            &src_claude,
            src_home.to_str().unwrap(),
            &[sel],
            &["node_modules".into()],
            &["settings.json".into(), ".credentials.json".into()], // el 2º debe ignorarse
            &zip_path,
            |_, cur, tot| prog.push((cur, tot)),
        )
        .unwrap();
        // progreso en bytes: total fijo > 0, monotónico, termina en total
        let tot = prog[0].1;
        assert!(tot > 0);
        assert!(prog.iter().all(|p| p.1 == tot));
        assert!(prog.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(*prog.last().unwrap(), (tot, tot));

        // PC destino simulado (otro home)
        let dst_home = tmp.join("dst-home");
        let dst_claude = dst_home.join(".claude");
        fs::create_dir_all(&dst_claude).unwrap();

        let preview = inspect_archive(&zip_path, &dst_claude, dst_home.to_str().unwrap()).unwrap();
        assert_eq!(preview.projects.len(), 1);
        let expected_target = format!("{}\\miproyecto", dst_home.to_str().unwrap());
        assert_eq!(preview.projects[0].suggested_target_path, expected_target);
        assert_eq!(preview.projects[0].file_count, 1); // node_modules excluido
        assert!(!preview.projects[0].sessions[0].exists_locally);
        // solo settings.json en el archivo; credentials ignorado
        assert_eq!(preview.config.len(), 1);
        assert_eq!(preview.config[0].rel_path, "settings.json");
        assert!(!preview.config[0].exists_locally);

        let res = ImportResolution {
            slug: slug.clone(),
            import_chats: true,
            session_mode: "merge".into(),
            import_files: true,
            only_newer: false,
            target_path: expected_target.clone(),
        };
        let cfg_res = ConfigResolution {
            rel_paths: vec!["settings.json".into()],
            backup_existing: false,
        };
        let mut iprog: Vec<(u64, u64)> = Vec::new();
        let summary = import_projects(
            &zip_path,
            &dst_claude,
            dst_home.to_str().unwrap(),
            &[res.clone()],
            &cfg_res,
            |_, cur, tot| iprog.push((cur, tot)),
        )
        .unwrap();
        let itot = iprog[0].1;
        assert!(itot > 0);
        assert!(iprog.iter().all(|p| p.1 == itot));
        assert!(iprog.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(*iprog.last().unwrap(), (itot, itot));
        assert_eq!(summary.sessions_imported, 1);
        assert_eq!(summary.files_written, 1);
        assert_eq!(summary.history_lines_added, 1);
        assert_eq!(summary.config_written, 1);

        // settings importado con el home remapeado al destino
        let settings = fs::read_to_string(dst_claude.join("settings.json")).unwrap();
        assert!(settings.contains(&dst_home.to_str().unwrap().replace('\\', "\\\\")));
        assert!(!settings.contains(&home_esc), "queda home antiguo en settings");
        // credentials nunca escrito en destino
        assert!(!dst_claude.join(".credentials.json").exists());

        // segunda importación con backup: settings existente se respalda
        let cfg_res2 = ConfigResolution {
            rel_paths: vec!["settings.json".into()],
            backup_existing: true,
        };
        import_projects(
            &zip_path,
            &dst_claude,
            dst_home.to_str().unwrap(),
            &[],
            &cfg_res2,
            |_, _, _| {},
        )
        .unwrap();
        let baks: Vec<_> = fs::read_dir(&dst_claude)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert_eq!(baks.len(), 1, "esperaba una copia .bak de settings");

        // chat remapeado en el slug nuevo, con cwd nuevo
        let new_slug = slug_for_path(&expected_target);
        let imported = fs::read_to_string(
            dst_claude.join("projects").join(&new_slug).join("abc-123.jsonl"),
        )
        .unwrap();
        let expected_esc = expected_target.replace('\\', "\\\\");
        assert!(imported.contains(&expected_esc), "cwd no remapeado: {imported}");
        assert!(!imported.contains(&esc), "queda ruta antigua: {imported}");

        // archivos en su sitio
        assert_eq!(
            fs::read_to_string(Path::new(&expected_target).join("main.py")).unwrap(),
            "print('hola')"
        );

        // tercera importación en modo merge: sesión saltada
        let summary2 = import_projects(
            &zip_path,
            &dst_claude,
            dst_home.to_str().unwrap(),
            &[res],
            &ConfigResolution::default(),
            |_, _, _| {},
        )
        .unwrap();
        assert_eq!(summary2.sessions_imported, 0);
        assert_eq!(summary2.sessions_skipped, 1);
        assert_eq!(summary2.history_lines_added, 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    /// E2E con los datos reales de ~/.claude de este PC. Solo lee los datos
    /// reales; escribe únicamente en un directorio temporal que simula otro
    /// PC con otro home. `cargo test -- --ignored real_data_e2e`
    #[test]
    #[ignore]
    fn real_data_e2e() {
        let home = std::env::var("USERPROFILE").expect("USERPROFILE");
        let claude = Path::new(&home).join(".claude");
        assert!(claude.is_dir(), "no hay ~/.claude en este PC");

        let excl: Vec<String> =
            ["node_modules", ".git", "target", "venv"].iter().map(|s| s.to_string()).collect();
        let projects = list_projects(&claude, &excl).unwrap();
        assert!(!projects.is_empty());
        // proyecto más pequeño con carpeta existente, para que el test sea rápido
        let p = projects
            .iter()
            .filter(|p| p.folder_exists)
            .min_by_key(|p| p.folder_size)
            .expect("ningún proyecto con carpeta");
        println!("proyecto elegido: {} ({} sesiones)", p.real_path, p.sessions.len());

        let tmp = std::env::temp_dir().join(format!("cctx-real-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let zip_path = tmp.join("export.cctx");

        let sel = ExportSelection {
            slug: p.slug.clone(),
            real_path: p.real_path.clone(),
            session_ids: p.sessions.iter().map(|s| s.id.clone()).collect(),
            include_files: true,
        };
        let cfg: Vec<String> = list_config(&claude).into_iter().map(|c| c.rel_path).collect();
        export_projects(
            &claude,
            &home,
            &[sel],
            &["node_modules".into(), ".git".into(), "target".into(), "venv".into()],
            &cfg,
            &zip_path,
            |_, _, _| {},
        )
        .unwrap();
        assert!(zip_path.metadata().unwrap().len() > 0);

        // PC destino simulado con otro home
        let dst_home = tmp.join("Users").join("OtroUsuario");
        let dst_claude = dst_home.join(".claude");
        fs::create_dir_all(&dst_claude).unwrap();
        let dst_home_s = dst_home.to_str().unwrap();

        let preview = inspect_archive(&zip_path, &dst_claude, dst_home_s).unwrap();
        let pp = &preview.projects[0];
        assert!(pp.suggested_target_path.starts_with(dst_home_s), "{}", pp.suggested_target_path);
        assert!(pp.sessions.iter().all(|s| !s.exists_locally));

        let res = ImportResolution {
            slug: p.slug.clone(),
            import_chats: true,
            session_mode: "merge".into(),
            import_files: true,
            only_newer: false,
            target_path: pp.suggested_target_path.clone(),
        };
        let cfg_res = ConfigResolution {
            rel_paths: preview.config.iter().map(|c| c.rel_path.clone()).collect(),
            backup_existing: false,
        };
        let summary =
            import_projects(&zip_path, &dst_claude, dst_home_s, &[res], &cfg_res, |_, _, _| {}).unwrap();
        assert_eq!(summary.sessions_imported, p.sessions.len() as u64);

        // ninguna referencia al home antiguo en los chats importados; el home
        // destino cuelga del real (Temp está bajo el home), así que primero se
        // neutraliza el destino y después se comprueba que no queda el antiguo
        let new_slug = slug_for_path(&pp.suggested_target_path);
        let chat_dir = dst_claude.join("projects").join(&new_slug);
        for f in fs::read_dir(&chat_dir).unwrap().flatten() {
            let content = fs::read_to_string(f.path()).unwrap();
            let sanitized = remap_content(&content, dst_home_s, "X:\\DST");
            let old_esc = home.replace('\\', "\\\\");
            assert!(
                !sanitized.contains(&old_esc),
                "quedan rutas del home antiguo en {:?}",
                f.file_name()
            );
        }
        // los proyectos importados aparecen al listar en el PC destino
        let dst_projects = list_projects(&dst_claude, &excl).unwrap();
        assert_eq!(dst_projects.len(), 1);
        assert_eq!(dst_projects[0].real_path, pp.suggested_target_path);

        println!("resumen: {summary:?}");
        let _ = fs::remove_dir_all(&tmp);
    }
}
