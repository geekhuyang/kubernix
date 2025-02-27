use crate::{
    config::Config,
    kubeconfig::KubeConfig,
    network::Network,
    process::{Process, Startable, Stoppable},
};
use failure::Fallible;
use log::info;
use std::fs::{self, create_dir_all};

pub struct Proxy {
    process: Process,
}

impl Proxy {
    pub fn start(
        config: &Config,
        network: &Network,
        kubeconfig: &KubeConfig,
    ) -> Fallible<Startable> {
        info!("Starting Proxy");

        let dir = config.root().join("proxy");
        create_dir_all(&dir)?;

        let yml = format!(
            include_str!("assets/proxy.yml"),
            kubeconfig.proxy().display(),
            network.cluster(),
        );
        let yml_file = dir.join("config.yml");
        fs::write(&yml_file, yml)?;

        let mut process = Process::start(
            config,
            &dir,
            "kube-proxy",
            &[&format!("--config={}", yml_file.display())],
        )?;

        process.wait_ready("Caches are synced")?;
        info!("Proxy is ready");
        Ok(Box::new(Proxy { process }))
    }
}

impl Stoppable for Proxy {
    fn stop(&mut self) -> Fallible<()> {
        self.process.stop()
    }
}
