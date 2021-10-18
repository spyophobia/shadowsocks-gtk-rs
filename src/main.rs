mod clap_def;
mod config_loader;
mod profile_manager;
mod tray;
mod util;

fn main() {
    gtk::init().unwrap();

    let _tray_item = tray::start();

    gtk::main();
}

#[cfg(test)]
mod test {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use crate::{
        config_loader::ConfigFolder,
        profile_manager::{OnFailure, ProfileManager},
        util::NaiveLeakyBucketConfig,
    };

    /// This test will always pass. You need to examine the outputs manually.
    ///
    /// `cargo test example_profiles_test_run -- --nocapture`
    #[test]
    fn example_profiles_test_run() {
        simple_logger::init().unwrap();

        // parse example configs
        let eg_configs = ConfigFolder::from_path_recurse("example_config").unwrap();
        let eg_configs = Box::leak(Box::new(eg_configs));
        let profile_list = eg_configs.get_profiles();
        println!("Loaded {:?} profiles.", profile_list.len());

        let on_fail = OnFailure::Restart {
            limit: NaiveLeakyBucketConfig::new(3, Duration::from_secs(10)),
        };
        let mut manager = ProfileManager::new(on_fail);
        let stdout = manager.stdout_rx.clone();
        let stderr = manager.stderr_rx.clone();
        thread::spawn(move || stdout.iter().for_each(|s| println!("stdout: {}", s)));
        thread::spawn(move || stderr.iter().for_each(|s| println!("stderr: {}", s)));

        for p in profile_list {
            manager.switch_to(p.clone()).unwrap();
            sleep(Duration::from_millis(2500));
        }
        let _ = manager.stop();
    }
}
