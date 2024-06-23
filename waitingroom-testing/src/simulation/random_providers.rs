use waitingroom_core::random::{DeterministicRandomProvider, RandomProvider};

pub struct RandomProviders {
    network_random_provider: DeterministicRandomProvider,
    node_random_provider: DeterministicRandomProvider,
    disturbance_random_provider: DeterministicRandomProvider,
    user_random_provider: DeterministicRandomProvider,
}

impl RandomProviders {
    pub fn new(seed: u64) -> Self {
        let base_random_provider = DeterministicRandomProvider::new(seed);

        let network_random_provider =
            DeterministicRandomProvider::new(base_random_provider.random_u64());
        let node_random_provider =
            DeterministicRandomProvider::new(base_random_provider.random_u64());
        let disturbance_random_provider =
            DeterministicRandomProvider::new(base_random_provider.random_u64());
        let user_random_provider =
            DeterministicRandomProvider::new(base_random_provider.random_u64());

        Self {
            network_random_provider,
            node_random_provider,
            disturbance_random_provider,
            user_random_provider,
        }
    }

    pub fn network_random_provider(&self) -> &DeterministicRandomProvider {
        &self.network_random_provider
    }

    pub fn node_random_provider(&self) -> &DeterministicRandomProvider {
        &self.node_random_provider
    }

    pub fn disturbance_random_provider(&self) -> &DeterministicRandomProvider {
        &self.disturbance_random_provider
    }

    pub fn user_random_provider(&self) -> &DeterministicRandomProvider {
        &self.user_random_provider
    }
}
