import { CSSProperties, ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArchivePreview,
  ConfigFile,
  CONFIG_LABELS,
  ConfigResolution,
  DEFAULT_EXCLUSIONS,
  ExportSelection,
  fmtSize,
  ImportResolution,
  ImportSummary,
  Progress,
  ProjectInfo,
} from "./types";
import "./App.css";

// ── paleta del diseño ──
const AC = "#d97757";
const OFF = "#3f382f";
const BD = "#4a443c";
const INK = "#1b120d";

const appWindow = getCurrentWindow();

const fmtDate = (ms: number) =>
  ms ? new Date(ms).toLocaleDateString("en-US", { day: "numeric", month: "short" }) : "";

type Tab = "export" | "import";

export default function App() {
  const [tab, setTab] = useState<Tab>("export");
  const [progress, setProgress] = useState<Progress | null>(null);

  useEffect(() => {
    const un = listen<Progress>("transfer-progress", (e) => setProgress(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  return (
    <div
      style={{
        height: "100vh",
        background: "#1e1b18",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      <TitleBar />
      <ViewSwitcher tab={tab} setTab={setTab} />
      {tab === "export" ? (
        <ExportView progress={progress} resetProgress={() => setProgress(null)} />
      ) : (
        <ImportView progress={progress} resetProgress={() => setProgress(null)} />
      )}
    </div>
  );
}

// ── barra de título (ventana sin marco) ──
function TitleBar() {
  const ctrl: CSSProperties = {
    width: 44,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    cursor: "pointer",
    color: "#8a8178",
    height: "100%",
  };
  return (
    <div
      data-tauri-drag-region
      style={{
        height: 40,
        flex: "none",
        display: "flex",
        alignItems: "center",
        background: "#23201c",
        borderBottom: "1px solid #2e2923",
        paddingLeft: 14,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 9, flex: 1, pointerEvents: "none" }}>
        <div
          style={{
            width: 19,
            height: 19,
            borderRadius: 5,
            background: AC,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flex: "none",
          }}
        >
          <svg width="11" height="11" viewBox="0 0 12 12">
            <path
              d="M1 4h7M6 1.5 8.5 4 6 6.5M11 8H4M6 5.5 3.5 8 6 10.5"
              stroke={INK}
              strokeWidth="1.5"
              fill="none"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>
        <span style={{ fontSize: 12.5, fontWeight: 600, color: "#c9c1b8", letterSpacing: 0.2 }}>
          Claude Code Transfer
        </span>
        <span className="mono" style={{ fontSize: 11, color: "#5f584f" }}>
          v1.0
        </span>
      </div>
      <div style={{ display: "flex", height: "100%" }}>
        <div style={ctrl} onClick={() => appWindow.minimize()} title="Minimize">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M1 5h8" stroke="currentColor" strokeWidth="1" />
          </svg>
        </div>
        <div style={ctrl} onClick={() => appWindow.toggleMaximize()} title="Maximize">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="1.5" y="1.5" width="7" height="7" stroke="currentColor" strokeWidth="1" fill="none" />
          </svg>
        </div>
        <div
          style={ctrl}
          onClick={() => appWindow.close()}
          title="Close"
          onMouseEnter={(e) => {
            e.currentTarget.style.background = "#b8543f";
            e.currentTarget.style.color = "#fff";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = "transparent";
            e.currentTarget.style.color = "#8a8178";
          }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M1.5 1.5 8.5 8.5M8.5 1.5 1.5 8.5" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </div>
      </div>
    </div>
  );
}

function ViewSwitcher({ tab, setTab }: { tab: Tab; setTab: (t: Tab) => void }) {
  const seg = (active: boolean): CSSProperties => ({
    padding: "6px 22px",
    borderRadius: 7,
    fontSize: 13,
    fontWeight: 600,
    cursor: "pointer",
    background: active ? "#33291f" : "transparent",
    color: active ? "#e0a189" : "#8a8178",
    transition: "background .15s,color .15s",
  });
  return (
    <div
      style={{
        height: 56,
        flex: "none",
        display: "flex",
        alignItems: "center",
        padding: "0 20px",
        gap: 16,
        borderBottom: "1px solid #2a2620",
      }}
    >
      <div
        style={{
          display: "flex",
          background: "#17140f",
          border: "1px solid #2e2923",
          borderRadius: 9,
          padding: 3,
          gap: 2,
        }}
      >
        <div style={seg(tab === "export")} onClick={() => setTab("export")}>
          Export
        </div>
        <div style={seg(tab === "import")} onClick={() => setTab("import")}>
          Import
        </div>
      </div>
      <div style={{ flex: 1 }} />
    </div>
  );
}

// ── piezas reutilizables ──
function Check({ on, half }: { on: boolean; half?: boolean }) {
  return (
    <div
      style={{
        width: 16,
        height: 16,
        borderRadius: 4,
        flex: "none",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: on || half ? AC : "transparent",
        border: `1px solid ${on || half ? AC : BD}`,
        transition: "background .12s",
      }}
    >
      {on && (
        <svg width="10" height="10" viewBox="0 0 10 10">
          <path d="M1.5 5.2 4 7.7 8.5 2.6" stroke={INK} strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
      {half && !on && <div style={{ width: 8, height: 2, background: INK, borderRadius: 1 }} />}
    </div>
  );
}

function Toggle({ on }: { on: boolean }) {
  return (
    <div
      style={{
        width: 30,
        height: 17,
        borderRadius: 9,
        background: on ? AC : OFF,
        position: "relative",
        transition: "background .15s",
        flex: "none",
      }}
    >
      <div
        style={{
          position: "absolute",
          top: 2,
          left: on ? 15 : 2,
          width: 13,
          height: 13,
          borderRadius: "50%",
          background: on ? "#f3ece3" : "#8a8178",
          transition: "left .15s",
        }}
      />
    </div>
  );
}

const card: CSSProperties = {
  flex: "none",
  background: "#26221e",
  border: "1px solid #302b25",
  borderRadius: 10,
};
const primaryBtn = (enabled: boolean): CSSProperties => ({
  padding: "8px 18px",
  borderRadius: 8,
  background: enabled ? AC : "#332e27",
  color: enabled ? INK : "#6f675f",
  fontSize: 13,
  fontWeight: 600,
  cursor: enabled ? "pointer" : "default",
  transition: "background .15s",
});
const ghostBtn: CSSProperties = {
  padding: "7px 12px",
  borderRadius: 8,
  border: "1px solid #35302a",
  fontSize: 12.5,
  fontWeight: 500,
  color: "#9c938a",
  cursor: "pointer",
};

// ════════════ EXPORTAR ════════════
function ExportView({ progress, resetProgress }: { progress: Progress | null; resetProgress: () => void }) {
  const [projects, setProjects] = useState<ProjectInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Record<string, Set<string>>>({});
  const [includeFiles, setIncludeFiles] = useState<Record<string, boolean>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [exclusions, setExclusions] = useState<Set<string>>(new Set(DEFAULT_EXCLUSIONS.filter((x) => x !== "target")));
  const [customList, setCustomList] = useState<string[]>([]);
  const [newExcl, setNewExcl] = useState("");
  const [exclOpen, setExclOpen] = useState(false);
  const [configFiles, setConfigFiles] = useState<ConfigFile[]>([]);
  const [configSel, setConfigSel] = useState<Set<string>>(new Set());
  const [cfgOpen, setCfgOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [donePath, setDonePath] = useState<string | null>(null);

  useEffect(() => {
    invoke<ProjectInfo[]>("list_projects", { exclusions: [...exclusions, ...customList] })
      .then(setProjects)
      .catch((e) => setError(String(e)));
    invoke<ConfigFile[]>("list_config").then(setConfigFiles).catch(() => {});
  }, []);

  // refresca tamaños de carpeta al cambiar exclusiones (con debounce)
  useEffect(() => {
    if (!projects) return;
    const excl = [...exclusions, ...customList];
    const paths = projects.map((p) => p.realPath);
    const t = setTimeout(() => {
      invoke<number[]>("folder_sizes", { paths, exclusions: excl })
        .then((sizes) =>
          setProjects((ps) =>
            ps &&
            ps.map((p) => {
              const i = paths.indexOf(p.realPath);
              return i >= 0 ? { ...p, folderSize: sizes[i] } : p;
            })
          )
        )
        .catch(() => {});
    }, 400);
    return () => clearTimeout(t);
  }, [exclusions, customList]);

  if (error)
    return <div style={{ padding: 24, color: "#e0876c" }}>Error reading projects: {error}</div>;
  if (!projects) return <div style={{ padding: 24, color: "#8a8178" }}>Loading projects…</div>;

  const toggleProject = (p: ProjectInfo) => {
    setSelected((s) => {
      const next = { ...s };
      if (next[p.slug]) delete next[p.slug];
      else next[p.slug] = new Set(p.sessions.map((x) => x.id));
      return next;
    });
    setIncludeFiles((f) => ({ ...f, [p.slug]: f[p.slug] ?? true }));
  };
  const toggleSession = (slug: string, id: string) =>
    setSelected((s) => {
      const cur = new Set(s[slug] ?? []);
      cur.has(id) ? cur.delete(id) : cur.add(id);
      const next = { ...s };
      cur.size === 0 ? delete next[slug] : (next[slug] = cur);
      return next;
    });

  let selSess = 0;
  let totSess = 0;
  let selProj = 0;
  let est = 0;
  for (const p of projects) {
    const n = selected[p.slug]?.size ?? 0;
    totSess += p.sessions.length;
    selSess += n;
    if (n > 0) {
      selProj++;
      const sel = selected[p.slug]!;
      est += p.sessions.reduce((a, s) => (sel.has(s.id) ? a + s.size : a), 0);
      if ((includeFiles[p.slug] ?? true) && p.folderExists) est += p.folderSize;
    }
  }
  for (const c of configFiles) if (configSel.has(c.relPath)) est += c.size;
  const allSel = selSess === totSess && totSess > 0;
  const someOnly = selSess > 0 && !allSel;
  const canExport = (selSess > 0 || configSel.size > 0) && !busy;
  const activeExcl = [...exclusions, ...customList];

  const toggleAll = () => {
    if (allSel) setSelected({});
    else {
      const s: Record<string, Set<string>> = {};
      for (const p of projects) s[p.slug] = new Set(p.sessions.map((x) => x.id));
      setSelected(s);
    }
  };

  const doExport = async () => {
    const dest = await save({
      title: "Save export",
      defaultPath: `transfer-${new Date().toISOString().slice(0, 10)}.cctx`,
      filters: [{ name: "Claude Code Transfer", extensions: ["cctx"] }],
    });
    if (!dest) return;
    const selections: ExportSelection[] = projects
      .filter((p) => selected[p.slug])
      .map((p) => ({
        slug: p.slug,
        realPath: p.realPath,
        sessionIds: [...selected[p.slug]],
        includeFiles: (includeFiles[p.slug] ?? true) && p.folderExists,
      }));
    resetProgress();
    setBusy(true);
    setDonePath(null);
    try {
      await invoke("export_projects", {
        selections,
        exclusions: activeExcl,
        configFiles: [...configSel],
        destPath: dest,
      });
      setDonePath(dest);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
      {/* barra superior */}
      <div style={{ flex: "none", display: "flex", alignItems: "center", gap: 14, padding: "12px 20px 10px" }}>
        <div
          onClick={toggleAll}
          style={{ display: "flex", alignItems: "center", gap: 9, cursor: "pointer", padding: "4px 6px", margin: "-4px -6px", borderRadius: 7 }}
        >
          <Check on={allSel} half={someOnly} />
          <span style={{ fontSize: 13, fontWeight: 500, color: "#c9c1b8" }}>Select all</span>
        </div>
        <div style={{ width: 1, height: 18, background: "#332e27" }} />
        <span style={{ fontSize: 12.5, color: "#9c938a" }}>
          {selProj} of {projects.length} projects · {selSess} sessions
        </span>
        <span style={{ fontSize: 12.5, color: "#9c938a" }}>·</span>
        <span className="mono" style={{ fontSize: 12.5, color: AC, fontWeight: 600 }}>
          ~{fmtSize(est)}
        </span>
        <div style={{ flex: 1 }} />
        <div
          onClick={() => setCfgOpen((v) => !v)}
          style={{ ...ghostBtn, display: "flex", alignItems: "center", gap: 6 }}
        >
          Configuration
          <span className="mono" style={{ background: "#33291f", color: AC, fontSize: 10.5, fontWeight: 700, padding: "1px 6px", borderRadius: 99 }}>
            {configSel.size}
          </span>
          <Chevron open={cfgOpen} />
        </div>
        <div
          onClick={() => setExclOpen((v) => !v)}
          style={{ ...ghostBtn, display: "flex", alignItems: "center", gap: 6 }}
        >
          Exclusions
          <span className="mono" style={{ background: "#33291f", color: AC, fontSize: 10.5, fontWeight: 700, padding: "1px 6px", borderRadius: 99 }}>
            {activeExcl.length}
          </span>
          <Chevron open={exclOpen} />
        </div>
        <div onClick={() => canExport && doExport()} style={primaryBtn(canExport)}>
          {busy ? "Exporting…" : "Export…"}
        </div>
      </div>

      {/* panel exclusiones */}
      {exclOpen && (
        <Panel title="Folders and patterns excluded when copying files">
          {DEFAULT_EXCLUSIONS.map((x) => (
            <Chip
              key={x}
              name={x}
              on={exclusions.has(x)}
              onToggle={() => {
                const n = new Set(exclusions);
                n.has(x) ? n.delete(x) : n.add(x);
                setExclusions(n);
              }}
            />
          ))}
          {customList.map((x) => (
            <Chip key={x} name={x} on custom onRemove={() => setCustomList((l) => l.filter((y) => y !== x))} onToggle={() => {}} />
          ))}
          <input
            value={newExcl}
            onChange={(e) => setNewExcl(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newExcl.trim()) {
                setCustomList((l) => [...l, newExcl.trim()]);
                setNewExcl("");
              }
            }}
            placeholder="add pattern and ⏎"
            className="mono"
            style={{ background: "#17140f", border: "1px dashed #3a342d", borderRadius: 6, color: "#ece7e0", fontSize: 12, padding: "5px 10px", outline: "none", width: 150 }}
          />
        </Panel>
      )}

      {/* panel configuración global */}
      {cfgOpen && (
        <Panel title="Global configuration (never includes credentials or caches)">
          {configFiles.length === 0 && <span style={{ fontSize: 12, color: "#6f675f" }}>No configuration files.</span>}
          {configFiles.map((c) => (
            <Chip
              key={c.relPath}
              name={`${CONFIG_LABELS[c.relPath] ?? c.relPath} · ${fmtSize(c.size)}`}
              on={configSel.has(c.relPath)}
              onToggle={() => {
                const n = new Set(configSel);
                n.has(c.relPath) ? n.delete(c.relPath) : n.add(c.relPath);
                setConfigSel(n);
              }}
            />
          ))}
        </Panel>
      )}

      {/* lista de proyectos */}
      <div style={{ flex: 1, overflowY: "auto", padding: "2px 20px 16px", display: "flex", flexDirection: "column", gap: 7, minHeight: 0 }}>
        {projects.map((p) => {
          const n = selected[p.slug]?.size ?? 0;
          const checked = n === p.sessions.length && n > 0;
          const some = n > 0 && !checked;
          const inc = includeFiles[p.slug] ?? true;
          const exp = !!expanded[p.slug];
          return (
            <div key={p.slug} style={{ ...card, borderColor: checked || some ? "#3d352c" : "#302b25" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 14px", cursor: "pointer" }} onClick={() => setExpanded((e) => ({ ...e, [p.slug]: !e[p.slug] }))}>
                <div onClick={(e) => { e.stopPropagation(); toggleProject(p); }} style={{ cursor: "pointer" }}>
                  <Check on={checked} half={some} />
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
                    <span style={{ fontSize: 13.5, fontWeight: 600, color: "#ece7e0" }}>{p.name}</span>
                    <span className="mono" style={{ fontSize: 11, color: "#6f675f", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.realPath}</span>
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: 6, marginTop: 3, fontSize: 11.5, color: "#8a8178" }}>
                    <span>{n === p.sessions.length || n === 0 ? `${p.sessions.length} sessions` : `${n} of ${p.sessions.length} sessions`}</span>
                    <span style={{ color: "#4a4339" }}>·</span>
                    <span className="mono">{fmtSize(p.chatSize)} chats</span>
                    <span style={{ color: "#4a4339" }}>·</span>
                    {p.folderExists ? (
                      <span className="mono">{fmtSize(p.folderSize)} folder</span>
                    ) : (
                      <span style={{ color: "#d9a94f", display: "flex", alignItems: "center", gap: 4 }}>
                        <WarnIcon />
                        folder not found
                      </span>
                    )}
                  </div>
                </div>
                {p.folderExists && (
                  <div onClick={(e) => { e.stopPropagation(); setIncludeFiles((f) => ({ ...f, [p.slug]: !inc })); }} style={{ display: "flex", alignItems: "center", gap: 7, flex: "none", cursor: "pointer", padding: "4px 6px", margin: "-4px 0", borderRadius: 7 }}>
                    <span style={{ fontSize: 11.5, color: inc ? "#c9c1b8" : "#6f675f", fontWeight: 500 }}>include files</span>
                    <Toggle on={inc} />
                  </div>
                )}
                <div style={{ flex: "none", color: "#6f675f", transform: exp ? "rotate(180deg)" : "none", transition: "transform .15s" }}>
                  <Chevron open={exp} plain />
                </div>
              </div>
              {exp && (
                <div style={{ borderTop: "1px solid #2e2923", padding: "6px 14px 8px", display: "flex", flexDirection: "column" }}>
                  {p.sessions.map((sx) => {
                    const on = selected[p.slug]?.has(sx.id) ?? false;
                    return (
                      <div key={sx.id} onClick={() => toggleSession(p.slug, sx.id)} style={{ display: "flex", alignItems: "center", gap: 10, padding: "5px 8px", margin: "0 -8px", borderRadius: 6, cursor: "pointer" }}>
                        <div style={{ width: 14, height: 14, borderRadius: 4, flex: "none", display: "flex", alignItems: "center", justifyContent: "center", background: on ? AC : "transparent", border: `1px solid ${on ? AC : BD}` }}>
                          {on && (
                            <svg width="9" height="9" viewBox="0 0 10 10">
                              <path d="M1.5 5.2 4 7.7 8.5 2.6" stroke={INK} strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
                            </svg>
                          )}
                        </div>
                        <span style={{ flex: 1, minWidth: 0, fontSize: 12.5, color: "#c9c1b8", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{sx.title}</span>
                        <span className="mono" style={{ flex: "none", fontSize: 11, color: "#6f675f" }}>{fmtSize(sx.size)}</span>
                        <span style={{ flex: "none", width: 48, textAlign: "right", fontSize: 11, color: "#6f675f" }}>{fmtDate(sx.modifiedMs)}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* progreso */}
      {busy && progress && (
        <div style={{ flex: "none", padding: "12px 20px 14px", borderTop: "1px solid #2a2620", background: "#221e1a" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 8 }}>
            <span className="mono" style={{ fontSize: 12.5, color: "#c9c1b8" }}>{progress.message}</span>
            <span className="mono" style={{ fontSize: 12, color: AC, fontWeight: 600 }}>
              {progress.total ? Math.round((progress.current / progress.total) * 100) : 0} %
            </span>
          </div>
          <Bar pct={progress.total ? (progress.current / progress.total) * 100 : 0} />
        </div>
      )}

      {/* banner éxito */}
      {donePath && (
        <div style={{ flex: "none", display: "flex", alignItems: "center", gap: 12, margin: "0 20px 14px", padding: "11px 14px", background: "rgba(143,191,127,.09)", border: "1px solid rgba(143,191,127,.3)", borderRadius: 10 }}>
          <div style={{ width: 22, height: 22, borderRadius: "50%", background: "#8fbf7f", display: "flex", alignItems: "center", justifyContent: "center", flex: "none" }}>
            <svg width="11" height="11" viewBox="0 0 10 10">
              <path d="M1.5 5.2 4 7.7 8.5 2.6" stroke="#16210f" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: "#b5d8a8" }}>Export complete</div>
            <div className="mono" style={{ fontSize: 11.5, color: "#8a9a80", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{donePath}</div>
          </div>
          <div onClick={() => setDonePath(null)} style={{ flex: "none", color: "#8a9a80", cursor: "pointer" }}>
            <svg width="11" height="11" viewBox="0 0 10 10">
              <path d="M1.5 1.5 8.5 8.5M8.5 1.5 1.5 8.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
            </svg>
          </div>
        </div>
      )}
    </div>
  );
}

// ════════════ IMPORTAR ════════════
function ImportView({ progress, resetProgress }: { progress: Progress | null; resetProgress: () => void }) {
  const [zipPath, setZipPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ArchivePreview | null>(null);
  const [resolutions, setResolutions] = useState<Record<string, ImportResolution>>({});
  const [configSel, setConfigSel] = useState<Set<string>>(new Set());
  const [configBackup, setConfigBackup] = useState(true);
  const [phase, setPhase] = useState<"empty" | "loaded" | "importing" | "done">("empty");
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pickFile = async () => {
    const path = await open({
      title: "Open export",
      filters: [{ name: "Claude Code Transfer", extensions: ["cctx", "zip"] }],
      multiple: false,
    });
    if (typeof path !== "string") return;
    setError(null);
    try {
      const p = await invoke<ArchivePreview>("inspect_archive", { zipPath: path });
      setZipPath(path);
      setPreview(p);
      const res: Record<string, ImportResolution> = {};
      for (const proj of p.projects)
        res[proj.slug] = {
          slug: proj.slug,
          importChats: true,
          sessionMode: "merge",
          importFiles: proj.includesFiles,
          onlyNewer: proj.targetFolderExists,
          targetPath: proj.suggestedTargetPath,
        };
      setResolutions(res);
      setConfigSel(new Set(p.config.map((c) => c.relPath)));
      setPhase("loaded");
    } catch (e) {
      setError(String(e));
    }
  };

  const upd = (slug: string, patch: Partial<ImportResolution>) =>
    setResolutions((r) => ({ ...r, [slug]: { ...r[slug], ...patch } }));

  const doImport = async () => {
    if (!zipPath) return;
    resetProgress();
    setPhase("importing");
    setError(null);
    try {
      const config: ConfigResolution = { relPaths: [...configSel], backupExisting: configBackup };
      const s = await invoke<ImportSummary>("import_projects", {
        zipPath,
        resolutions: Object.values(resolutions).filter((r) => r.importChats || r.importFiles),
        config,
      });
      setSummary(s);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("loaded");
    }
  };

  const reset = () => {
    setPhase("empty");
    setPreview(null);
    setZipPath(null);
    setSummary(null);
  };

  const srcName = (p: string) => p.split(/[\\/]/).pop() || p;

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
      {error && <div style={{ padding: "10px 20px", color: "#e0876c", fontSize: 12.5 }}>Error: {error}</div>}

      {phase === "empty" && (
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", padding: 32 }}>
          <div onClick={pickFile} style={{ width: 520, padding: "56px 40px", border: "1.5px dashed #3f382f", borderRadius: 14, display: "flex", flexDirection: "column", alignItems: "center", gap: 14, cursor: "pointer" }}>
            <div style={{ width: 52, height: 52, borderRadius: 14, background: "#2b2721", display: "flex", alignItems: "center", justifyContent: "center" }}>
              <svg width="24" height="24" viewBox="0 0 24 24">
                <path d="M12 16V5M8 8.5 12 4.5l4 4" stroke={AC} strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />
                <path d="M4 15v3.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V15" stroke="#8a8178" strokeWidth="1.8" fill="none" strokeLinecap="round" />
              </svg>
            </div>
            <div style={{ fontSize: 15, fontWeight: 600, color: "#ece7e0" }}>Open a .cctx file</div>
            <div style={{ fontSize: 12.5, color: "#8a8178", marginTop: -6 }}>click to select it from disk</div>
          </div>
        </div>
      )}

      {phase === "loaded" && preview && (
        <>
          <div style={{ flex: "none", display: "flex", alignItems: "center", gap: 14, padding: "12px 20px 10px" }}>
            <div style={{ width: 34, height: 34, borderRadius: 9, background: "#33291f", display: "flex", alignItems: "center", justifyContent: "center", flex: "none" }}>
              <svg width="16" height="16" viewBox="0 0 16 16">
                <path d="M4 1.5h5.5L13 5v8.5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1Z" stroke={AC} strokeWidth="1.3" fill="none" strokeLinejoin="round" />
                <path d="M9.5 1.5V5H13" stroke={AC} strokeWidth="1.3" fill="none" strokeLinejoin="round" />
              </svg>
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="mono" style={{ fontSize: 13.5, fontWeight: 600, color: "#ece7e0", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{srcName(zipPath || "")}</div>
              <div style={{ fontSize: 11.5, color: "#8a8178", marginTop: 2 }}>
                Source: <span style={{ color: "#c9c1b8" }} className="mono">{preview.sourceHome}</span> · {preview.projects.length} projects
                {preview.config.length > 0 && ` · ${preview.config.length} config files`}
              </div>
            </div>
            <div onClick={reset} style={ghostBtn}>Change file</div>
            <div onClick={doImport} style={primaryBtn(true)}>Import</div>
          </div>

          <div style={{ flex: 1, overflowY: "auto", padding: "2px 20px 16px", display: "flex", flexDirection: "column", gap: 7, minHeight: 0 }}>
            {preview.projects.map((c) => {
              const r = resolutions[c.slug];
              if (!r) return null;
              return (
                <div key={c.slug} style={{ ...card, padding: "12px 14px" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <span style={{ fontSize: 13.5, fontWeight: 600, color: "#ece7e0" }}>{srcName(c.sourcePath)}</span>
                    <span className="mono" style={{ fontSize: 11, color: "#6f675f", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1, minWidth: 0 }}>{c.sourcePath}</span>
                    {c.existingSessionCount > 0 && <Badge text={`${c.existingSessionCount} ${c.existingSessionCount === 1 ? "session already exists" : "sessions already exist"}`} />}
                    {c.targetFolderExists && <Badge text="target folder already exists" />}
                  </div>
                  <div style={{ fontSize: 11.5, color: "#8a8178", marginTop: 3 }}>
                    {c.sessions.length} sessions · {c.includesFiles ? `${c.fileCount} files · ${fmtSize(c.filesSize)}` : "no files in package"}
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: 18, marginTop: 11, paddingTop: 11, borderTop: "1px solid #2e2923", flexWrap: "wrap" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <div onClick={() => upd(c.slug, { importChats: !r.importChats })} style={{ cursor: "pointer" }}>
                        <Toggle on={r.importChats} />
                      </div>
                      <span style={{ fontSize: 12.5, fontWeight: 500, color: "#c9c1b8" }}>chats</span>
                      {r.importChats && c.existingSessionCount > 0 && (
                        <div style={{ display: "flex", background: "#17140f", border: "1px solid #2e2923", borderRadius: 7, padding: 2, gap: 2, marginLeft: 2 }}>
                          {(["merge", "overwrite"] as const).map((m) => (
                            <div key={m} onClick={() => upd(c.slug, { sessionMode: m })} style={{ padding: "3px 10px", borderRadius: 5, fontSize: 11.5, fontWeight: 600, cursor: "pointer", background: r.sessionMode === m ? "#33291f" : "transparent", color: r.sessionMode === m ? "#e0a189" : "#8a8178" }}>
                              {m === "merge" ? "merge" : "overwrite"}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                    {c.includesFiles && (
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <div onClick={() => upd(c.slug, { importFiles: !r.importFiles })} style={{ cursor: "pointer" }}>
                          <Toggle on={r.importFiles} />
                        </div>
                        <span style={{ fontSize: 12.5, fontWeight: 500, color: "#c9c1b8" }}>files</span>
                        {r.importFiles && c.targetFolderExists && (
                          <div onClick={() => upd(c.slug, { onlyNewer: !r.onlyNewer })} style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer", marginLeft: 2, padding: "3px 6px", borderRadius: 6 }}>
                            <Check on={r.onlyNewer} />
                            <span style={{ fontSize: 11.5, color: "#9c938a" }}>only newer</span>
                          </div>
                        )}
                      </div>
                    )}
                    <div style={{ display: "flex", alignItems: "center", gap: 8, flex: 1, minWidth: 260, justifyContent: "flex-end" }}>
                      <span style={{ fontSize: 11.5, color: "#8a8178", flex: "none" }}>destination</span>
                      <input value={r.targetPath} onChange={(e) => upd(c.slug, { targetPath: e.target.value })} className="mono" style={{ flex: 1, maxWidth: 340, background: "#17140f", border: "1px solid #2e2923", borderRadius: 7, color: "#c9c1b8", fontSize: 11.5, padding: "6px 10px", outline: "none" }} />
                    </div>
                  </div>
                </div>
              );
            })}

            {preview.config.length > 0 && (
              <div style={{ ...card, padding: "12px 14px" }}>
                <div style={{ fontSize: 12.5, fontWeight: 600, color: "#ece7e0", marginBottom: 8 }}>Global configuration</div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
                  {preview.config.map((cf) => (
                    <div
                      key={cf.relPath}
                      onClick={() => {
                        const n = new Set(configSel);
                        n.has(cf.relPath) ? n.delete(cf.relPath) : n.add(cf.relPath);
                        setConfigSel(n);
                      }}
                      style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", padding: "5px 8px", borderRadius: 7, background: "#221e1a" }}
                    >
                      <Check on={configSel.has(cf.relPath)} />
                      <span style={{ fontSize: 12, color: "#c9c1b8" }}>{CONFIG_LABELS[cf.relPath] ?? cf.relPath}</span>
                      {cf.existsLocally && <span style={{ fontSize: 11, color: "#d9a94f" }}>⚠ already exists</span>}
                    </div>
                  ))}
                </div>
                <div onClick={() => setConfigBackup((v) => !v)} style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", marginTop: 10 }}>
                  <Check on={configBackup} />
                  <span style={{ fontSize: 11.5, color: "#9c938a" }}>back up existing files (.bak) before overwriting</span>
                </div>
              </div>
            )}
          </div>
        </>
      )}

      {phase === "importing" && (
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", padding: 32 }}>
          <div style={{ width: 520 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <span className="mono" style={{ fontSize: 13, color: "#c9c1b8" }}>{progress?.message ?? "Importing…"}</span>
              <span className="mono" style={{ fontSize: 12, color: AC, fontWeight: 600 }}>
                {progress?.total ? Math.round((progress.current / progress.total) * 100) : 0} %
              </span>
            </div>
            <Bar pct={progress?.total ? (progress.current / progress.total) * 100 : 0} />
            <div style={{ fontSize: 11.5, color: "#6f675f", marginTop: 10, textAlign: "center" }}>Do not close the app during import</div>
          </div>
        </div>
      )}

      {phase === "done" && summary && (
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", padding: 32 }}>
          <div style={{ width: 460, display: "flex", flexDirection: "column", alignItems: "center", gap: 18 }}>
            <div style={{ width: 52, height: 52, borderRadius: "50%", background: "rgba(143,191,127,.14)", border: "1px solid rgba(143,191,127,.35)", display: "flex", alignItems: "center", justifyContent: "center" }}>
              <svg width="22" height="22" viewBox="0 0 10 10">
                <path d="M1.5 5.2 4 7.7 8.5 2.6" stroke="#8fbf7f" strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
            <div style={{ fontSize: 17, fontWeight: 700, color: "#ece7e0" }}>Import complete</div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, width: "100%" }}>
              <Stat n={summary.sessionsImported} label="sessions imported" />
              <Stat n={summary.sessionsSkipped} label="sessions skipped (already existed)" dim />
              <Stat n={summary.filesWritten} label="files written" />
              <Stat n={summary.filesSkipped} label="files skipped (older)" dim />
              <Stat n={summary.configWritten} label="config files" />
              <Stat n={summary.historyLinesAdded} label="history lines" dim />
            </div>
            <div onClick={reset} style={primaryBtn(true)}>Done</div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── piezas menores ──
function Chevron({ open, plain }: { open: boolean; plain?: boolean }) {
  return (
    <svg width={plain ? 11 : 9} height={plain ? 11 : 9} viewBox="0 0 10 10" style={{ transform: plain ? undefined : `rotate(${open ? 180 : 0}deg)`, transition: "transform .15s" }}>
      <path d="M2 3.5 5 6.5 8 3.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div style={{ flex: "none", margin: "0 20px 10px", padding: "12px 14px", background: "#221e1a", border: "1px solid #2e2923", borderRadius: 10 }}>
      <div style={{ fontSize: 11, fontWeight: 600, color: "#8a8178", textTransform: "uppercase", letterSpacing: 0.7, marginBottom: 9 }}>{title}</div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>{children}</div>
    </div>
  );
}

function Chip({ name, on, custom, onToggle, onRemove }: { name: string; on: boolean; custom?: boolean; onToggle: () => void; onRemove?: () => void }) {
  return (
    <div onClick={onToggle} className="mono" style={{ display: "flex", alignItems: "center", gap: 6, padding: "4px 10px", borderRadius: 6, fontSize: 12, cursor: "pointer", background: on ? "rgba(217,119,87,.12)" : "transparent", color: on ? "#e0a189" : "#8a8178", border: `1px solid ${on ? "rgba(217,119,87,.35)" : "#3a342d"}` }}>
      {on && (
        <svg width="9" height="9" viewBox="0 0 10 10">
          <path d="M1.5 5.2 4 7.7 8.5 2.6" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
      {name}
      {custom && onRemove && (
        <svg onClick={(e) => { e.stopPropagation(); onRemove(); }} width="9" height="9" viewBox="0 0 10 10" style={{ opacity: 0.6, cursor: "pointer" }}>
          <path d="M1.5 1.5 8.5 8.5M8.5 1.5 1.5 8.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      )}
    </div>
  );
}

function Badge({ text }: { text: string }) {
  return (
    <span style={{ flex: "none", display: "flex", alignItems: "center", gap: 5, fontSize: 11, fontWeight: 600, color: "#d9a94f", background: "rgba(217,169,79,.1)", border: "1px solid rgba(217,169,79,.28)", padding: "2.5px 8px", borderRadius: 99 }}>
      <WarnIcon />
      {text}
    </span>
  );
}

function WarnIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 12 12">
      <path d="M6 1 11 10H1Z" stroke="#d9a94f" strokeWidth="1.2" fill="none" strokeLinejoin="round" />
      <path d="M6 4.5v2.7" stroke="#d9a94f" strokeWidth="1.2" strokeLinecap="round" />
      <circle cx="6" cy="8.9" r=".7" fill="#d9a94f" />
    </svg>
  );
}

function Bar({ pct }: { pct: number }) {
  return (
    <div style={{ height: 6, borderRadius: 3, background: "#17140f", overflow: "hidden" }}>
      <div style={{ height: "100%", width: `${Math.max(0, Math.min(100, pct))}%`, borderRadius: 3, background: "linear-gradient(90deg,#c96a4a,#e08a6c)", transition: "width .1s linear" }} />
    </div>
  );
}

function Stat({ n, label, dim }: { n: number; label: string; dim?: boolean }) {
  return (
    <div style={{ background: "#26221e", border: "1px solid #302b25", borderRadius: 10, padding: "14px 16px" }}>
      <div className="mono" style={{ fontSize: 22, fontWeight: 700, color: dim ? "#9c938a" : "#ece7e0" }}>{n}</div>
      <div style={{ fontSize: 11.5, color: "#8a8178", marginTop: 2 }}>{label}</div>
    </div>
  );
}
