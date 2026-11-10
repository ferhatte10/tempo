// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Talks to the owner's systemd (`systemctl --user`) over D-Bus, read only: it
//! lists the timers and what each says, and tells when something changed.
//! Nothing here starts, stops, enables or writes anything.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{Stream, StreamExt};
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath, proxy};

use crate::files::{self, Pair};
use crate::journal;
use crate::snapshot::{self, Clocks, RawService, RawTimer, Texts};
use crate::view::TimerView;

const SERVICE: &str = "org.freedesktop.systemd1";

/// `name`, `description`, `load state`, `active state`, `sub state`,
/// `followed`, `object path`, `job id`, `job type`, `job path`.
type UnitStatus = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Manager {
    /// `(file path, state)` of the unit files matching the patterns.
    fn list_unit_files_by_patterns(
        &self,
        states: &[&str],
        patterns: &[&str],
    ) -> zbus::Result<Vec<(String, String)>>;

    fn list_units_by_patterns(
        &self,
        states: &[&str],
        patterns: &[&str],
    ) -> zbus::Result<Vec<UnitStatus>>;

    /// Fails for a unit that is not loaded.
    fn get_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;

    /// Asks for the signals about units, for this connection only.
    fn subscribe(&self) -> zbus::Result<()>;

    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// `(carries install info, changes)`.
    #[allow(clippy::type_complexity)]
    fn enable_unit_files(
        &self,
        files: &[&str],
        runtime: bool,
        force: bool,
    ) -> zbus::Result<(bool, Vec<(String, String, String)>)>;

    fn disable_unit_files(
        &self,
        files: &[&str],
        runtime: bool,
    ) -> zbus::Result<Vec<(String, String, String)>>;

    /// Makes systemd read the unit files again.
    fn reload(&self) -> zbus::Result<()>;

    fn get_unit_file_state(&self, name: &str) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait Unit {
    #[zbus(property)]
    fn active_state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn description(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn fragment_path(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Timer",
    default_service = "org.freedesktop.systemd1"
)]
trait Timer {
    #[zbus(property, name = "LastTriggerUSec")]
    fn last_trigger_usec(&self) -> zbus::Result<u64>;
    #[zbus(property, name = "NextElapseUSecRealtime")]
    fn next_elapse_realtime(&self) -> zbus::Result<u64>;
    #[zbus(property, name = "NextElapseUSecMonotonic")]
    fn next_elapse_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn unit(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1"
)]
trait Service {
    #[zbus(property)]
    fn result(&self) -> zbus::Result<String>;
    #[zbus(property, name = "ExecMainStatus")]
    fn exec_main_status(&self) -> zbus::Result<i32>;
    #[zbus(property, name = "ExecMainStartTimestamp")]
    fn exec_main_start(&self) -> zbus::Result<u64>;
    #[zbus(property, name = "ExecMainExitTimestamp")]
    fn exec_main_exit(&self) -> zbus::Result<u64>;
}

#[derive(Clone)]
pub struct Systemd {
    conn: Connection,
}

/// What is known about one timer before its properties are read.
#[derive(Default)]
struct Entry {
    file: Option<String>,
    /// `enabled`, `disabled`, `static`, `masked`…
    file_state: String,
    loaded: Option<(String, OwnedObjectPath)>,
}

/// Where unit files of the user manager live, the owner's first.
fn unit_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(config) = config_dir() {
        dirs.push(config.join("systemd/user"));
    }
    for dir in [
        "/etc/systemd/user",
        "/run/systemd/user",
        "/usr/local/lib/systemd/user",
        "/usr/lib/systemd/user",
        "/lib/systemd/user",
    ] {
        dirs.push(PathBuf::from(dir));
    }
    dirs
}

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config")))
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn find_unit_file(name: &str) -> Option<String> {
    unit_dirs()
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

fn clocks() -> Clocks {
    let realtime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_micros()).unwrap_or(0));
    let monotonic = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let micros = u64::try_from(monotonic.tv_sec).unwrap_or(0) * 1_000_000
        + u64::try_from(monotonic.tv_nsec).unwrap_or(0) / 1_000;
    Clocks {
        realtime_usec: realtime,
        monotonic_usec: micros,
    }
}

impl Systemd {
    pub async fn connect() -> zbus::Result<Self> {
        Ok(Self {
            conn: Connection::session().await?,
        })
    }

    /// How the last run of a loaded service ended: systemd's `Result`
    /// (`exit-code`, `timeout`…) and its exit status.
    pub async fn last_result(&self, service: &str) -> zbus::Result<(String, i32)> {
        let manager = ManagerProxy::new(&self.conn).await?;
        let path = manager.get_unit(service).await?;
        let proxy = ServiceProxy::builder(&self.conn)
            .path(path)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        Ok((proxy.result().await?, proxy.exec_main_status().await?))
    }

    /// Every timer of the owner's systemd, as the interface shows it.
    pub async fn timers(&self) -> zbus::Result<Vec<TimerView>> {
        let manager = ManagerProxy::new(&self.conn).await?;
        let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
        for (path, state) in manager
            .list_unit_files_by_patterns(&[], &["*.timer"])
            .await?
        {
            if let Some(name) = std::path::Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
            {
                let entry = entries.entry(name.to_owned()).or_default();
                entry.file = Some(path.clone());
                entry.file_state = state;
            }
        }
        for (name, description, _load, _active, _sub, _followed, path, ..) in
            manager.list_units_by_patterns(&[], &["*.timer"]).await?
        {
            entries.entry(name).or_default().loaded = Some((description, path));
        }

        let clocks = clocks();
        let home = home_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        let mut raws = Vec::new();
        for (name, entry) in entries {
            raws.push(self.raw_timer(&manager, &name, &entry).await);
        }
        let services: Vec<String> = raws.iter().map(|raw| raw.activates.clone()).collect();
        let mut history = journal::history(&services, clocks.realtime_usec).await;
        let mut views = Vec::new();
        for raw in raws {
            let texts = Texts {
                timer: raw.fragment_path.as_deref().and_then(read),
                service: raw
                    .service
                    .as_ref()
                    .and_then(|service| service.fragment_path.clone())
                    .or_else(|| find_unit_file(&raw.activates))
                    .as_deref()
                    .and_then(read),
            };
            let runs = history.remove(&raw.activates).unwrap_or_default();
            views.push(snapshot::to_view(&raw, &texts, clocks, &home, &runs));
        }
        snapshot::sort(&mut views);
        Ok(views)
    }

    async fn raw_timer(&self, manager: &ManagerProxy<'_>, name: &str, entry: &Entry) -> RawTimer {
        let base = name.strip_suffix(".timer").unwrap_or(name);
        let mut raw = RawTimer {
            unit: name.to_owned(),
            fragment_path: entry.file.clone(),
            active_state: "inactive".to_owned(),
            unit_file_state: entry.file_state.clone(),
            activates: format!("{base}.service"),
            ..RawTimer::default()
        };
        let Some((description, path)) = &entry.loaded else {
            return raw;
        };
        raw.description.clone_from(description);

        let build = async |connection: &Connection| {
            let unit = UnitProxy::builder(connection)
                .path(path.clone())
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;
            let timer = TimerProxy::builder(connection)
                .path(path.clone())
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;
            zbus::Result::Ok((unit, timer))
        };
        let Ok((unit, timer)) = build(&self.conn).await else {
            return raw;
        };
        raw.active_state = unit.active_state().await.unwrap_or_default();
        if let Ok(file) = unit.fragment_path().await
            && !file.is_empty()
        {
            raw.fragment_path = Some(file);
        }
        raw.last_trigger_usec = timer.last_trigger_usec().await.unwrap_or(0);
        raw.next_realtime_usec = timer.next_elapse_realtime().await.unwrap_or(0);
        raw.next_monotonic_usec = timer.next_elapse_monotonic().await.unwrap_or(0);
        if let Ok(service) = timer.unit().await
            && !service.is_empty()
        {
            raw.activates = service;
        }
        raw.service = self.raw_service(manager, &raw.activates).await;
        raw
    }

    /// The service a timer starts, when systemd still has it loaded.
    async fn raw_service(&self, manager: &ManagerProxy<'_>, name: &str) -> Option<RawService> {
        let path = manager.get_unit(name).await.ok()?;
        let unit = UnitProxy::builder(&self.conn)
            .path(path.clone())
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .ok()?;
        let service = ServiceProxy::builder(&self.conn)
            .path(path)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .ok()?;
        Some(RawService {
            fragment_path: unit.fragment_path().await.ok().filter(|p| !p.is_empty()),
            active_state: unit.active_state().await.unwrap_or_default(),
            result: service.result().await.unwrap_or_default(),
            exec_main_status: service.exec_main_status().await.unwrap_or(0),
            start_usec: service.exec_main_start().await.unwrap_or(0),
            exit_usec: service.exec_main_exit().await.unwrap_or(0),
        })
    }

    /// Yields an item whenever a timer or a service may have changed.
    pub async fn changes(&self) -> zbus::Result<impl Stream<Item = ()> + use<>> {
        // Without this, systemd keeps the unit signals to itself. It is
        // per connection and ends with it.
        let _ = ManagerProxy::new(&self.conn).await?.subscribe().await;
        let signal = || {
            MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(SERVICE)
        };
        let rules = [
            signal().interface("org.freedesktop.systemd1.Manager"),
            signal()
                .interface("org.freedesktop.DBus.Properties")
                .member("PropertiesChanged"),
        ];
        let mut streams = Vec::new();
        for rule in rules {
            streams.push(MessageStream::for_match_rule(rule.build()?, &self.conn, None).await?);
        }
        Ok(
            futures_util::stream::select_all(streams).filter_map(|message| {
                // Properties change on every unit all the time: only a timer or a
                // service (the `.` of its name is written `_2e` in object paths)
                // is worth a new look.
                let relevant = message.ok().is_some_and(|message| {
                    let header = message.header();
                    header.path().is_none_or(|path| {
                        let path = path.as_str();
                        !path.starts_with("/org/freedesktop/systemd1/unit/")
                            || path.ends_with("_2etimer")
                            || path.ends_with("_2eservice")
                    })
                });
                std::future::ready(relevant.then_some(()))
            }),
        )
    }

    /// Reads a unit's active state, for a unit that may not be loaded.
    async fn active_state(&self, name: &str) -> Option<String> {
        let manager = ManagerProxy::new(&self.conn).await.ok()?;
        let path = manager.get_unit(name).await.ok()?;
        UnitProxy::builder(&self.conn)
            .path(path)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .ok()?
            .active_state()
            .await
            .ok()
    }
}

/// A copy of a deleted timer, to bring it back.
#[derive(Debug, Clone)]
pub struct Saved {
    pub name: String,
    pub title: String,
    pair: Pair,
    enabled: bool,
    active: bool,
    /// The copy of the file Tempo kept for it, when small enough to bring back.
    kept: Option<Vec<files::KeptFile>>,
}

fn failed(err: impl std::fmt::Display) -> String {
    err.to_string()
}

/// What the owner can do to a timer Tempo wrote. Each one first checks that
/// the files are Tempo's, in the owner's folder, and refuses otherwise.
impl Systemd {
    async fn checked(
        &self,
        unit: &str,
    ) -> Result<(String, tempo_core::spec::TimerSpec, Pair), String> {
        let name = unit.strip_suffix(".timer").unwrap_or(unit).to_owned();
        let dir = files::user_dir().ok_or("no home folder")?;
        let (spec, pair) = files::managed(&dir, &name)?;
        Ok((name, spec, pair))
    }

    /// Starts the service now, without waiting for the timer.
    pub async fn run_now(&self, unit: &str) -> Result<(), String> {
        let (name, ..) = self.checked(unit).await?;
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager
            .start_unit(&format!("{name}.service"), "replace")
            .await
            .map(|_| ())
            .map_err(failed)
    }

    /// Stops the timer until the next session; it stays enabled.
    pub async fn pause(&self, unit: &str) -> Result<(), String> {
        let (name, ..) = self.checked(unit).await?;
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager
            .stop_unit(&format!("{name}.timer"), "replace")
            .await
            .map(|_| ())
            .map_err(failed)
    }

    pub async fn resume(&self, unit: &str) -> Result<(), String> {
        let (name, ..) = self.checked(unit).await?;
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager
            .start_unit(&format!("{name}.timer"), "replace")
            .await
            .map(|_| ())
            .map_err(failed)
    }

    /// Stops the timer and keeps it from starting with the next session.
    pub async fn turn_off(&self, unit: &str) -> Result<(), String> {
        let (name, ..) = self.checked(unit).await?;
        let timer = format!("{name}.timer");
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager
            .disable_unit_files(&[timer.as_str()], false)
            .await
            .map_err(failed)?;
        manager.reload().await.map_err(failed)?;
        manager
            .stop_unit(&timer, "replace")
            .await
            .map(|_| ())
            .map_err(failed)
    }

    /// Enables the timer for the next sessions and starts it now.
    pub async fn turn_on(&self, unit: &str) -> Result<(), String> {
        let (name, ..) = self.checked(unit).await?;
        let timer = format!("{name}.timer");
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager
            .enable_unit_files(&[timer.as_str()], false, true)
            .await
            .map_err(failed)?;
        manager.reload().await.map_err(failed)?;
        manager
            .start_unit(&timer, "replace")
            .await
            .map(|_| ())
            .map_err(failed)
    }

    /// Stops the timer, turns it off and removes its two files. The returned
    /// copy brings it back with [`Systemd::restore`].
    pub async fn delete(&self, unit: &str) -> Result<Saved, String> {
        let (name, spec, pair) = self.checked(unit).await?;
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        let timer = format!("{name}.timer");
        let service = format!("{name}.service");
        let enabled = manager
            .get_unit_file_state(&timer)
            .await
            .is_ok_and(|state| state.starts_with("enabled"));
        let active = self.active_state(&timer).await.as_deref() == Some("active");
        let kept = files::scripts_dir().and_then(|scripts| files::read_kept(&scripts, &name));
        let saved = Saved {
            name: name.clone(),
            title: spec.description,
            pair,
            enabled,
            active,
            kept,
        };

        // A unit that is not loaded or not enabled makes these fail: nothing
        // to undo, so the errors are not the user's concern.
        let _ = manager.stop_unit(&timer, "replace").await;
        let _ = manager.stop_unit(&service, "replace").await;
        let _ = manager.disable_unit_files(&[timer.as_str()], false).await;
        let dir = files::user_dir().ok_or("no home folder")?;
        files::remove_pair(&dir, &name).map_err(failed)?;
        // A kept copy that was remembered goes too; a bigger one stays.
        if saved.kept.is_some()
            && let Some(scripts) = files::scripts_dir()
        {
            let _ = files::remove_kept(&scripts, &name);
        }
        manager.reload().await.map_err(failed)?;
        Ok(saved)
    }

    /// Writes a deleted timer back and puts it as it was.
    pub async fn restore(&self, saved: &Saved) -> Result<(), String> {
        let dir = files::user_dir().ok_or("no home folder")?;
        if let (Some(kept), Some(scripts)) = (&saved.kept, files::scripts_dir()) {
            files::write_kept(&scripts, &saved.name, kept).map_err(failed)?;
        }
        files::write_pair(&dir, &saved.name, &saved.pair).map_err(failed)?;
        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        manager.reload().await.map_err(failed)?;
        let timer = format!("{}.timer", saved.name);
        if saved.enabled {
            manager
                .enable_unit_files(&[timer.as_str()], false, true)
                .await
                .map_err(failed)?;
            manager.reload().await.map_err(failed)?;
        }
        if saved.active {
            manager
                .start_unit(&timer, "replace")
                .await
                .map_err(failed)?;
        }
        Ok(())
    }
}

/// Whether `name.timer` or `name.service` already exists, for the system or
/// for the owner.
fn name_taken(name: &str) -> bool {
    ["timer", "service"]
        .iter()
        .any(|kind| find_unit_file(&format!("{name}.{kind}")).is_some())
}

/// `new` with every occurrence of `from` as a whole word replaced by `to`.
fn move_path(
    command: &mut tempo_core::spec::Command,
    from: &std::path::Path,
    to: &std::path::Path,
) {
    if command.program == from {
        command.program = to.to_path_buf();
    }
    let (from, to) = (from.display().to_string(), to.display().to_string());
    for arg in &mut command.args {
        if *arg == from {
            to.clone_into(arg);
        }
    }
}

/// Asks `systemd-analyze --user verify` whether the two unit files are good,
/// from a scratch folder, without installing anything. Its remarks about other
/// units are none of the owner's concern: only those about these files count.
async fn verify(name: &str, pair: &Pair) -> Result<(), String> {
    use gtk::gio;
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!(
        "tempo-verify-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    files::write_pair(&dir, name, pair).map_err(failed)?;
    let timer = dir.join(format!("{name}.timer"));
    let service = dir.join(format!("{name}.service"));
    let run = async {
        let process = journal::spawn_program(
            "systemd-analyze",
            &[
                "--user".to_owned(),
                "verify".to_owned(),
                timer.display().to_string(),
                service.display().to_string(),
            ],
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
        )?;
        let (output, _) = process
            .communicate_utf8_future(None::<String>)
            .await
            .map_err(failed)?;
        Ok::<String, String>(output.map(String::from).unwrap_or_default())
    };
    let output = run.await;
    let _ = std::fs::remove_dir_all(&dir);
    let output = output?;
    let prefix = format!("{}/", dir.display());
    let remarks: Vec<String> = output
        .lines()
        .filter(|line| line.contains(&prefix) || line.contains(&format!("{name}.")))
        .map(|line| line.replace(&prefix, ""))
        .collect();
    if remarks.is_empty() {
        Ok(())
    } else {
        Err(remarks.join("\n"))
    }
}

/// What one run of a command said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryResult {
    pub exit_code: i32,
    pub output: String,
}

/// Longest part of the output that is kept: its end.
const MAX_TRY_OUTPUT: usize = 4_096;
/// A try that lasts longer is stopped.
const TRY_SECONDS: u32 = 60;

/// Runs the command once, now, as the timer would (through the owner's
/// systemd, `systemd-run --user`), and says how it went. The caller asks the
/// owner first: this runs their file.
pub async fn try_now(command: &tempo_core::spec::Command) -> Result<TryResult, String> {
    use gtk::gio;
    let mut args: Vec<String> = [
        "--user",
        "--pipe",
        "--wait",
        "--collect",
        "--quiet",
        "-p",
        "Type=oneshot",
        "-p",
    ]
    .map(str::to_owned)
    .to_vec();
    args.push(format!("RuntimeMaxSec={TRY_SECONDS}"));
    if let Some(dir) = &command.working_dir {
        args.push("-p".to_owned());
        args.push(format!("WorkingDirectory={}", dir.display()));
    }
    args.push("--".to_owned());
    args.push(command.program.display().to_string());
    args.extend(command.args.iter().cloned());
    let process = journal::spawn_program(
        "systemd-run",
        &args,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    )?;
    let (output, _) = process
        .communicate_utf8_future(None::<String>)
        .await
        .map_err(failed)?;
    let output = output.map(String::from).unwrap_or_default();
    let tail = if output.len() > MAX_TRY_OUTPUT {
        let mut start = output.len() - MAX_TRY_OUTPUT;
        while !output.is_char_boundary(start) {
            start += 1;
        }
        format!("…{}", &output[start..])
    } else {
        output
    };
    Ok(TryResult {
        exit_code: if process.has_exited() {
            process.exit_status()
        } else {
            -1
        },
        output: tail,
    })
}

impl Systemd {
    /// Creates a timer (`editing` false) or changes one of Tempo's. With
    /// `copy_from`, the file is first copied into Tempo's folder and the
    /// command points to the copy. The unit files are checked by systemd
    /// before anything is installed, and put back as they were if a step
    /// fails.
    pub async fn save(
        &self,
        spec: &tempo_core::spec::TimerSpec,
        copy_from: Option<&std::path::Path>,
        editing: bool,
    ) -> Result<(), String> {
        let dir = files::user_dir().ok_or("no home folder")?;
        let name = spec.name.as_str();
        let before = if editing {
            Some(files::managed(&dir, name)?.1)
        } else {
            if name_taken(name) {
                return Err("a timer or a service with this name already exists".to_owned());
            }
            None
        };

        let mut spec = spec.clone();
        let mut copied = None;
        if let Some(source) = copy_from {
            let scripts = files::scripts_dir().ok_or("no home folder")?;
            let kept = files::keep_copy(source, &scripts, name).map_err(failed)?;
            move_path(&mut spec.command, source, &kept);
            copied = Some(scripts);
        }
        let undo_copy = |copied: &Option<PathBuf>| {
            // A copy made for a new timer goes if the timer is not made.
            if let (Some(scripts), false) = (copied, editing) {
                let _ = files::remove_kept(scripts, name);
            }
        };

        let pair = Pair {
            timer: tempo_core::unit::render_timer(&spec).map_err(failed)?,
            service: tempo_core::unit::render_service(&spec).map_err(failed)?,
        };
        if let Err(err) = verify(name, &pair).await {
            undo_copy(&copied);
            return Err(err);
        }

        let manager = ManagerProxy::new(&self.conn).await.map_err(failed)?;
        let timer = format!("{name}.timer");
        let installed = async {
            if spec.notify_failure {
                // Points to this program, wherever it is now.
                let program = std::env::current_exe().map_err(failed)?;
                files::write_notifier(&dir, &program).map_err(failed)?;
            }
            files::write_pair(&dir, name, &pair).map_err(failed)?;
            manager.reload().await.map_err(failed)?;
            if before.is_some() {
                // New schedule, new conditions: the running timer starts over.
                if self.active_state(&timer).await.as_deref() == Some("active") {
                    manager
                        .restart_unit(&timer, "replace")
                        .await
                        .map_err(failed)?;
                }
            } else {
                manager
                    .enable_unit_files(&[timer.as_str()], false, true)
                    .await
                    .map_err(failed)?;
                manager.reload().await.map_err(failed)?;
                manager
                    .start_unit(&timer, "replace")
                    .await
                    .map_err(failed)?;
            }
            Ok::<(), String>(())
        }
        .await;
        if let Err(err) = installed {
            // Back to how it was.
            match &before {
                Some(old) => {
                    let _ = files::write_pair(&dir, name, old);
                }
                None => {
                    let _ = manager.disable_unit_files(&[timer.as_str()], false).await;
                    let _ = files::remove_pair(&dir, name);
                }
            }
            let _ = manager.reload().await;
            undo_copy(&copied);
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use tempo_core::schedule::Schedule;
    use tempo_core::spec::{Command, TimerSpec};
    use tempo_core::unit::{render_service, render_timer};

    use super::*;
    use crate::view::{Origin, State};

    const NAME: &str = "tempo-live-test";

    /// Removes the test timer whatever happens, with the tools of the system.
    struct Cleanup;

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let timer = format!("{NAME}.timer");
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", &timer])
                .output();
            if let Some(dir) = files::user_dir() {
                let _ = files::remove_pair(&dir, NAME);
            }
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output();
        }
    }

    fn settle() {
        std::thread::sleep(Duration::from_millis(700));
    }

    async fn find(systemd: &Systemd) -> Option<TimerView> {
        let unit = format!("{NAME}.timer");
        systemd
            .timers()
            .await
            .expect("timers")
            .into_iter()
            .find(|timer| timer.unit == unit)
    }

    /// Writes a harmless timer into the real user folder, drives it through
    /// every action with the real systemd, and removes it. It touches the
    /// owner's systemd, so it only runs when asked:
    /// `cargo test -p tempo live_actions -- --ignored`.
    #[test]
    #[ignore = "uses the real user systemd"]
    fn live_actions_on_a_tempo_timer() {
        let dir = files::user_dir().expect("home folder");
        assert!(
            !dir.join(format!("{NAME}.timer")).exists(),
            "{NAME} already exists: not touching it"
        );
        let _cleanup = Cleanup;
        let spec = TimerSpec {
            name: NAME.to_owned(),
            description: "Tempo live test".to_owned(),
            command: Command {
                program: PathBuf::from("/usr/bin/true"),
                args: Vec::new(),
                working_dir: None,
            },
            schedule: Schedule::Every { minutes: 120 },
            catch_up: false,
            random_delay_secs: 0,
            conditions: Vec::new(),
            low_priority: false,
            timeout_secs: None,
            notify_failure: false,
        };
        let saved = Saved {
            name: NAME.to_owned(),
            title: spec.description.clone(),
            pair: Pair {
                timer: render_timer(&spec).expect("renders"),
                service: render_service(&spec).expect("renders"),
            },
            enabled: true,
            active: true,
            kept: None,
        };
        let unit = format!("{NAME}.timer");

        let context = gtk::glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    let systemd = Systemd::connect().await.expect("systemd");
                    systemd.restore(&saved).await.expect("installed");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert_eq!(timer.origin, Origin::Yours);
                    assert_eq!(timer.title, "Tempo live test");
                    assert_eq!(timer.state, State::Waiting);
                    assert!(timer.enabled);
                    assert_eq!(timer.schedule, Schedule::Every { minutes: 120 });
                    assert!(timer.next_in_secs.is_some());

                    systemd.run_now(&unit).await.expect("run now");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert!(timer.last_run().is_some(), "it ran: {timer:?}");
                    // The journal holds the run: its history says it succeeded.
                    let mut history = Vec::new();
                    for _ in 0..10 {
                        history =
                            journal::history(&[format!("{NAME}.service")], clocks().realtime_usec)
                                .await
                                .remove(&format!("{NAME}.service"))
                                .unwrap_or_default();
                        if !history.is_empty() {
                            break;
                        }
                        settle();
                    }
                    assert_eq!(
                        history.last().map(|run| run.result),
                        Some(crate::view::RunResult::Success),
                        "history: {history:?}"
                    );

                    systemd.pause(&unit).await.expect("pause");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert_eq!((timer.state, timer.enabled), (State::Paused, true));
                    assert_eq!(timer.next_in_secs, None);

                    systemd.resume(&unit).await.expect("resume");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert_eq!((timer.state, timer.enabled), (State::Waiting, true));

                    systemd.turn_off(&unit).await.expect("turn off");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert_eq!((timer.state, timer.enabled), (State::Paused, false));

                    systemd.turn_on(&unit).await.expect("turn on");
                    settle();
                    let timer = find(&systemd).await.expect("listed");
                    assert_eq!((timer.state, timer.enabled), (State::Waiting, true));

                    let copy = systemd.delete(&unit).await.expect("delete");
                    settle();
                    assert!(find(&systemd).await.is_none(), "it is gone");
                    assert!(!dir.join(&unit).exists());
                    assert!(copy.enabled && copy.active, "it was remembered as running");

                    systemd.restore(&copy).await.expect("undo");
                    settle();
                    let timer = find(&systemd).await.expect("it is back");
                    assert_eq!((timer.state, timer.enabled), (State::Waiting, true));

                    // Only a timer Tempo wrote can be touched.
                    assert!(systemd.pause("systemd-tmpfiles-clean.timer").await.is_err());
                    assert!(
                        systemd
                            .delete("launchpadlib-cache-clean.timer")
                            .await
                            .is_err()
                    );
                    assert!(systemd.run_now("../x.timer").await.is_err());

                    systemd.delete(&unit).await.expect("final delete");
                })
            })
            .expect("main context");
    }

    const SAVE_NAME: &str = "tempo-live-save";
    const COPY_NAME: &str = "tempo-live-copy";

    /// Removes the timers and kept copies of the save test.
    struct SaveCleanup;

    impl Drop for SaveCleanup {
        fn drop(&mut self) {
            for name in [SAVE_NAME, COPY_NAME] {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "disable", "--now", &format!("{name}.timer")])
                    .output();
                if let Some(dir) = files::user_dir() {
                    let _ = files::remove_pair(&dir, name);
                }
                if let Some(scripts) = files::scripts_dir() {
                    let _ = files::remove_kept(&scripts, name);
                }
            }
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output();
        }
    }

    fn timer_spec(name: &str, program: &str) -> TimerSpec {
        TimerSpec {
            name: name.to_owned(),
            description: "Live save test".to_owned(),
            command: Command {
                program: PathBuf::from(program),
                args: Vec::new(),
                working_dir: None,
            },
            schedule: Schedule::Every { minutes: 120 },
            catch_up: false,
            random_delay_secs: 0,
            conditions: Vec::new(),
            low_priority: false,
            timeout_secs: None,
            notify_failure: false,
        }
    }

    async fn find_named(systemd: &Systemd, name: &str) -> Option<TimerView> {
        let unit = format!("{name}.timer");
        systemd
            .timers()
            .await
            .expect("timers")
            .into_iter()
            .find(|timer| timer.unit == unit)
    }

    /// Creates, changes, refuses and tries timers with the real systemd, in the
    /// owner's folders, and removes them. Run on demand:
    /// `cargo test -p tempo live_save -- --ignored`.
    #[test]
    #[ignore = "uses the real user systemd"]
    fn live_save_and_try() {
        let dir = files::user_dir().expect("home folder");
        for name in [SAVE_NAME, COPY_NAME] {
            assert!(
                !dir.join(format!("{name}.timer")).exists(),
                "{name} already exists: not touching it"
            );
        }
        let _cleanup = SaveCleanup;
        let scratch = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp"
        ))
        .join(format!("live-save-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("scratch");
        let script = scratch.join("job.sh");
        std::fs::write(&script, "#!/bin/sh\necho from the copy\n").expect("script");
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("mode");

        let context = gtk::glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    let systemd = Systemd::connect().await.expect("systemd");

                    // Create.
                    let spec = timer_spec(SAVE_NAME, "/usr/bin/true");
                    systemd.save(&spec, None, false).await.expect("created");
                    settle();
                    let timer = find_named(&systemd, SAVE_NAME).await.expect("listed");
                    assert_eq!(
                        (timer.origin, timer.state, timer.enabled),
                        (Origin::Yours, State::Waiting, true)
                    );

                    // The same name again is refused, and so is a system name.
                    assert!(systemd.save(&spec, None, false).await.is_err());
                    let taken = timer_spec("systemd-tmpfiles-clean", "/usr/bin/true");
                    assert!(systemd.save(&taken, None, false).await.is_err());

                    // Change it: title, schedule, priority; the timer stays active.
                    let mut changed = spec.clone();
                    changed.description = "Live save test, changed".to_owned();
                    changed.schedule = Schedule::Daily {
                        hour: 3,
                        minute: 30,
                    };
                    changed.low_priority = true;
                    systemd.save(&changed, None, true).await.expect("changed");
                    settle();
                    let timer = find_named(&systemd, SAVE_NAME).await.expect("listed");
                    assert_eq!(timer.title, "Live save test, changed");
                    assert_eq!(
                        timer.schedule,
                        Schedule::Daily {
                            hour: 3,
                            minute: 30
                        }
                    );
                    assert_eq!(timer.state, State::Waiting);

                    // A program that does not exist is stopped by systemd's own check.
                    let broken = timer_spec(COPY_NAME, "/nonexistent/tempo-program");
                    let err = systemd
                        .save(&broken, None, false)
                        .await
                        .expect_err("refused");
                    assert!(!err.is_empty());
                    assert!(!dir.join(format!("{COPY_NAME}.timer")).exists());

                    // Keep a copy of a file somewhere unstable.
                    let mut with_file = timer_spec(COPY_NAME, &script.display().to_string());
                    with_file.description = "Live copy test".to_owned();
                    systemd
                        .save(&with_file, Some(&script), false)
                        .await
                        .expect("created with a copy");
                    settle();
                    let scripts = files::scripts_dir().expect("data folder");
                    let kept = scripts.join(COPY_NAME).join("job.sh");
                    assert!(kept.exists(), "the copy is kept");
                    let timer = find_named(&systemd, COPY_NAME).await.expect("listed");
                    assert_eq!(timer.command, kept.display().to_string());

                    // Deleting it takes the copy, and the undo brings both back.
                    let copy = systemd
                        .delete(&format!("{COPY_NAME}.timer"))
                        .await
                        .expect("deleted");
                    assert!(!kept.exists(), "the copy went with it");
                    systemd.restore(&copy).await.expect("undone");
                    settle();
                    assert!(kept.exists(), "the copy is back");
                    assert!(find_named(&systemd, COPY_NAME).await.is_some());

                    // Trying a command says how it went.
                    let ok = try_now(&timer_spec("x", "/usr/bin/true").command)
                        .await
                        .expect("ran");
                    assert_eq!(ok.exit_code, 0);
                    let bad = try_now(&timer_spec("x", "/usr/bin/false").command)
                        .await
                        .expect("ran");
                    assert_eq!(bad.exit_code, 1);
                    let mut echo = timer_spec("x", "/bin/echo").command;
                    echo.args = vec!["hello from the try".to_owned()];
                    let said = try_now(&echo).await.expect("ran");
                    assert_eq!(said.exit_code, 0);
                    assert!(said.output.contains("hello from the try"), "{said:?}");
                    let missing = try_now(&timer_spec("x", "/nonexistent/prog").command)
                        .await
                        .expect("ran");
                    assert_ne!(missing.exit_code, 0);

                    systemd
                        .delete(&format!("{COPY_NAME}.timer"))
                        .await
                        .expect("cleaned");
                    systemd
                        .delete(&format!("{SAVE_NAME}.timer"))
                        .await
                        .expect("cleaned");
                })
            })
            .expect("main context");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
