// TEMPORARY CI diagnostic — reports why is_running() stays true after a kill.
use chrono::Utc;
use process_manager::engine;
use process_manager::state::{ManagedProcess, ProcessRuntime, StateStore};
use process_manager::{HealthCheck, RestartPolicy, Task};
use std::collections::HashMap;
use std::sync::Arc;

fn k0(t: &str) -> String {
    match std::process::Command::new("kill").arg("-0").arg(t).output() {
        Ok(o) => format!("{}", o.status.success()),
        Err(e) => format!("err({e})"),
    }
}

fn procstate(pid: i32) -> String {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => {
            // pid comm state ppid pgrp session
            let f: Vec<&str> = s.split_whitespace().collect();
            let cmd = std::fs::read(format!("/proc/{pid}/cmdline"))
                .map(|c| {
                    c.split(|b| *b == 0)
                        .filter(|x| !x.is_empty())
                        .map(|x| String::from_utf8_lossy(x).to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            format!(
                "state={} ppid={} pgrp={} session={} cmd=[{}]",
                f.get(2).unwrap_or(&"?"),
                f.get(3).unwrap_or(&"?"),
                f.get(4).unwrap_or(&"?"),
                f.get(5).unwrap_or(&"?"),
                cmd
            )
        }
        Err(e) => format!("stat_err({})", e.kind()),
    }
}

fn kill_run(args: &[&str]) -> String {
    match std::process::Command::new("kill").args(args).output() {
        Ok(o) => format!(
            "kill {} -> status={:?} stderr={}",
            args.join(" "),
            o.status.code(),
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => format!("kill {} -> err({e})", args.join(" ")),
    }
}

#[tokio::test]
async fn ci_probe_is_running_after_kill() {
    let mut report = String::new();
    let tmp = std::env::temp_dir().join(format!("cip-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let store = Arc::new(StateStore::new(tmp.clone()).unwrap());
    let task = Task {
        id: "probe".into(),
        command: "sleep 120".into(),
        args: vec![],
        working_dir: std::env::temp_dir(),
        env: HashMap::new(),
        is_detached: true,
        log_file: None,
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: true,
            max_restarts: 5,
            restart_window_secs: 300,
            initial_backoff_secs: 0,
            max_backoff_secs: 1,
            backoff_multiplier: 1.0,
            restart_on_exit_codes: None,
        },
    };
    let running = engine::spawn(&task).await.unwrap();
    drop(running.output_rx);
    let m = ManagedProcess {
        id: "probe".into(),
        pid: running.pid,
        pgid: running.pgid,
        task,
        start_time: Utc::now(),
        metadata: HashMap::new(),
        runtime: ProcessRuntime::default(),
    };
    store.save(&m).unwrap();

    let snap = |store: &StateStore, m: &ManagedProcess, tag: &str| {
        format!(
            "{tag} pid_k0={} pgid_k0={} {} is_running={}",
            k0(&m.pid.to_string()),
            m.pgid.map(|g| k0(&format!("-{}", g))).unwrap_or("-".into()),
            procstate(m.pid as i32),
            store.is_running(m),
        )
    };

    report.push_str(&format!("pid={} pgid={:?}\n", m.pid, m.pgid));
    report.push_str(&snap(&store, &m, "AVANT "));
    report.push('\n');

    let res = engine::terminate(m.pid, m.pgid, true).await.unwrap();
    report.push_str(&format!("terminate={res}\n"));
    report.push_str(&kill_run(&["-0", &format!("-{}", m.pid)]));
    report.push('\n');
    report.push_str(&kill_run(&["-9", &format!("-{}", m.pid)]));
    report.push('\n');
    for i in 0..4 {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        report.push_str(&snap(&store, &m, &format!("T+{:?}ms", (i + 1) * 400)));
        report.push('\n');
    }
    report.push_str(&kill_run(&["-9", &m.pid.to_string()]));
    report.push('\n');
    report.push_str(&snap(&store, &m, "PARPID "));
    report.push('\n');

    let mut st = 0;
    let wr = unsafe { libc::waitpid(m.pid as i32, &mut st, libc::WNOHANG) };
    report.push_str(&format!("waitpid={wr}\n"));
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    report.push_str(&snap(&store, &m, "POST "));

    let _ = engine::terminate(m.pid, m.pgid, true).await;
    panic!("CIPROBE\n{report}");
}
