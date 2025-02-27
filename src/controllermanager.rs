use crate::{
    config::Config,
    kubeconfig::KubeConfig,
    network::Network,
    pki::Pki,
    process::{Process, Startable, Stoppable},
};
use failure::Fallible;
use log::info;
use std::fs::create_dir_all;

pub struct ControllerManager {
    process: Process,
}

impl ControllerManager {
    pub fn start(
        config: &Config,
        network: &Network,
        pki: &Pki,
        kubeconfig: &KubeConfig,
    ) -> Fallible<Startable> {
        info!("Starting Controller Manager");

        let dir = config.root().join("controllermanager");
        create_dir_all(&dir)?;

        let mut process = Process::start(
            config,
            &dir,
            "kube-controller-manager",
            &[
                "--bind-address=0.0.0.0",
                &format!("--cluster-cidr={}", network.cluster()),
                "--cluster-name=kubernetes",
                &format!("--cluster-signing-cert-file={}", pki.ca().cert().display()),
                &format!("--cluster-signing-key-file={}", pki.ca().key().display()),
                &format!("--kubeconfig={}", kubeconfig.controller_manager().display()),
                "--leader-elect=false",
                &format!("--root-ca-file={}", pki.ca().cert().display()),
                &format!(
                    "--service-account-private-key-file={}",
                    pki.service_account().key().display()
                ),
                &format!("--service-cluster-ip-range={}", network.service()),
                "--use-service-account-credentials=true",
                "--v=2",
            ],
        )?;

        process.wait_ready("Serving securely")?;
        info!("Controller Manager is ready");
        Ok(Box::new(ControllerManager { process }))
    }
}

impl Stoppable for ControllerManager {
    fn stop(&mut self) -> Fallible<()> {
        self.process.stop()
    }
}
