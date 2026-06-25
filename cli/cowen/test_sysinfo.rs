use sysinfo::System;

fn main() {
    println!("Refreshing PID 0...");
    let mut sys = System::new();
    let sys_pid = sysinfo::Pid::from_u32(0);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
    println!("Done!");
}
