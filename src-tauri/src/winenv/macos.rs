//! Kullanıcı ortamı (macOS): PATH için kabuk profilinde (`~/.zprofile`, varsa `~/.bash_profile`) işaretli blok,
//! Finder sağ tık menüsü için Hızlı Eylemler (`~/Library/Services/*.workflow`). Yönetici izni gerekmez.

use crate::i18n::t;
use crate::platform::home;
use anyhow::Result;
use std::path::{Path, PathBuf};

const BEGIN: &str = "# >>> KaradelikDev PATH (otomatik, elle düzenlemeyin)";
const END: &str = "# <<< KaradelikDev PATH";

fn profiles() -> Vec<PathBuf> {
    let h = home();
    let mut v = vec![h.join(".zprofile")];
    let bash = h.join(".bash_profile");
    if bash.exists() || std::env::var("SHELL").map(|s| s.ends_with("bash")).unwrap_or(false) {
        v.push(bash);
    }
    v
}

/// Profil içeriğinden KaradelikDev bloğunu çıkarır.
fn strip_block(content: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in content.lines() {
        if line.trim() == BEGIN {
            inside = true;
            // Blok eklenirken araya konan boş satır da kaldırılır (dosya eski haline döner)
            if out.ends_with("\n\n") {
                out.pop();
            }
            continue;
        }
        if line.trim() == END {
            inside = false;
            continue;
        }
        if !inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn block(dirs: &[String]) -> String {
    let joined = dirs.iter().map(|d| d.replace('"', "\\\"")).collect::<Vec<_>>().join(":");
    format!("{BEGIN}\nexport PATH=\"{joined}:$PATH\"\n{END}\n")
}

/// Bloktaki dizinler (yoksa boş).
fn block_dirs(content: &str) -> Vec<String> {
    let mut inside = false;
    for line in content.lines() {
        let l = line.trim();
        if l == BEGIN {
            inside = true;
        } else if l == END {
            inside = false;
        } else if inside {
            if let Some(v) = l.strip_prefix("export PATH=\"").and_then(|r| r.strip_suffix(":$PATH\"")) {
                return v.split(':').map(str::to_string).collect();
            }
        }
    }
    vec![]
}

fn write_profile(file: &Path, dirs: Option<&[String]>) -> Result<()> {
    let cur = std::fs::read_to_string(file).unwrap_or_default();
    let mut new = strip_block(&cur);
    if let Some(d) = dirs {
        if !new.is_empty() && !new.ends_with("\n\n") {
            new.push('\n');
        }
        new.push_str(&block(d));
    }
    if new.trim().is_empty() && !file.exists() {
        return Ok(());
    }
    if new != cur {
        std::fs::write(file, new)?;
    }
    Ok(())
}

pub fn user_path() -> Vec<String> {
    profiles().iter().find_map(|f| std::fs::read_to_string(f).ok().map(|c| block_dirs(&c)).filter(|v| !v.is_empty())).unwrap_or_default()
}

/// KaradelikDev dizinlerini kabuk profillerine ekler (eski blok değiştirilir). Yeni açılan terminallerde geçerli olur.
pub fn add_to_path() -> Result<()> {
    let dirs: Vec<String> = crate::terminal::path_dirs().iter().map(|d| d.display().to_string()).collect();
    for f in profiles() {
        write_profile(&f, Some(&dirs))?;
    }
    Ok(())
}

pub fn in_path() -> bool {
    profiles().iter().any(|f| std::fs::read_to_string(f).map(|c| c.contains(BEGIN)).unwrap_or(false))
}

pub fn remove_from_path() -> Result<()> {
    for f in profiles() {
        if f.exists() {
            write_profile(&f, None)?;
        }
    }
    Ok(())
}

/// Otomatik başlatma kaydı (autostart eklentisinin LaunchAgent'ı).
pub fn remove_autostart() {
    let _ = std::fs::remove_file(home().join("Library").join("LaunchAgents").join("KaradelikDev.plist"));
}

pub fn path_report() -> String {
    let mut s = String::new();
    for f in profiles() {
        s.push_str(&format!("{}\n", f.display()));
    }
    s.push('\n');
    let ours = user_path();
    for p in &ours {
        s.push_str(&format!("★ {p}\n"));
    }
    if ours.is_empty() {
        s.push_str("—\n");
    }
    s
}

// ---------------------------------------------------------------- Finder Hızlı Eylemleri

fn services_dir() -> PathBuf {
    home().join("Library").join("Services")
}

fn workflow_terminal() -> PathBuf {
    services_dir().join("KaradelikDev Terminal.workflow")
}

fn workflow_editor() -> PathBuf {
    services_dir().join("KaradelikDev Editor.workflow")
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn uuid() -> String {
    use rand::Rng;
    let mut r = rand::rng();
    let h = |n: usize, r: &mut rand::rngs::ThreadRng| (0..n).map(|_| format!("{:X}", r.random_range(0..16u8))).collect::<String>();
    format!("{}-{}-{}-{}-{}", h(8, &mut r), h(4, &mut r), h(4, &mut r), h(4, &mut r), h(12, &mut r))
}

/// Finder'da klasörlere sağ tıklanınca (Hızlı Eylemler / Servisler) görünen, kabuk betiği çalıştıran iş akışı.
fn write_workflow(dir: &Path, title: &str, script: &str) -> Result<()> {
    let contents = dir.join("Contents");
    std::fs::create_dir_all(&contents)?;
    let info = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{title}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.folder</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#,
        title = xml(title)
    );
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>528</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{script}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>{u1}</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>{u2}</string>
				<key>UUID</key>
				<string>{u3}</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
			</dict>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleID</key>
		<string>com.apple.finder</string>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>applicationPaths</key>
		<array>
			<string>/System/Library/CoreServices/Finder.app</string>
		</array>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#,
        script = xml(script),
        u1 = uuid(),
        u2 = uuid(),
        u3 = uuid(),
    );
    std::fs::write(contents.join("Info.plist"), info)?;
    std::fs::write(contents.join("document.wflow"), doc)?;
    Ok(())
}

/// Servis menüsü önbelleğini yeniler (yeni Hızlı Eylem hemen görünsün).
fn refresh_services() {
    let mut c = crate::util::hidden("/System/Library/CoreServices/pbs");
    c.arg("-flush");
    let _ = crate::util::run_capture(c);
}

/// "KaradelikDev Terminal" ve (editör ayarlıysa) "KaradelikDev Editör" Finder Hızlı Eylemleri.
pub fn add_context_menu() -> Result<()> {
    let exe = std::env::current_exe()?;
    let q = |s: &str| crate::platform::sh_quote(s);
    let script = format!("for f in \"$@\"; do {} --terminal \"$f\"; done", q(&exe.display().to_string()));
    write_workflow(&workflow_terminal(), &t("ctx.terminal"), &script)?;
    let _ = std::fs::remove_dir_all(workflow_editor());
    if let Some(open) = editor_command() {
        write_workflow(&workflow_editor(), &t("ctx.editor"), &format!("for f in \"$@\"; do {open} \"$f\"; done"))?;
    }
    refresh_services();
    Ok(())
}

/// Klasörü editörde açan komut: Tercihler'deki editör (`.app` ise `open -a`), yoksa VS Code.
fn editor_command() -> Option<String> {
    let q = |s: &str| crate::platform::sh_quote(s);
    let editor = crate::kcore::settings().editor;
    let e = editor.trim();
    if !e.is_empty() {
        return Some(if e.ends_with(".app") { format!("/usr/bin/open -a {}", q(e)) } else { q(e) });
    }
    let code = Path::new("/Applications/Visual Studio Code.app");
    code.exists().then(|| format!("/usr/bin/open -a {}", q(&code.display().to_string())))
}

pub fn remove_context_menu() -> Result<()> {
    let _ = std::fs::remove_dir_all(workflow_terminal());
    let _ = std::fs::remove_dir_all(workflow_editor());
    refresh_services();
    Ok(())
}

pub fn context_menu_installed() -> bool {
    workflow_terminal().join("Contents").join("document.wflow").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_block() {
        let base = "eval \"$(/opt/homebrew/bin/brew shellenv)\"\n";
        let b = format!("{}\n{}", base, block(&["/a/b".into(), "/c".into()]));
        assert_eq!(block_dirs(&b), vec!["/a/b", "/c"]);
        assert_eq!(strip_block(&b), base);
        assert!(block_dirs(base).is_empty());
    }
}
