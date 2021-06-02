//! Module for starting peers and networks. Used only for tests

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    unused_results,
    missing_docs,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc,
    clippy::restriction
)]

use std::{
    convert::TryFrom,
    fmt::Debug,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use iroha::{
    config::Configuration, permissions::PermissionsValidatorBox, prelude::*,
    sumeragi::config::SumeragiConfiguration, torii::config::ToriiConfiguration,
};
use iroha_client::{client::Client, config::Configuration as ClientConfiguration};
use iroha_data_model::peer::Peer as DataModelPeer;
use iroha_data_model::prelude::*;
use iroha_error::{Error, Result};
use iroha_logger::config::{LevelEnv, LoggerConfiguration};
use iroha_logger::InstrumentFutures;
use rand::seq::SliceRandom;
use tempfile::TempDir;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Copy)]
pub struct ShutdownRuntime;

/// Network of peers
#[derive(Clone, Debug)]
pub struct Network {
    /// Genesis peer which sends genesis block to everyone
    pub genesis: Peer,
    /// peers
    pub peers: Vec<Peer>,
}

/// Peer structure
#[derive(Clone, Debug)]
pub struct Peer {
    /// id of peer
    pub id: PeerId,
    /// api address
    pub api_address: String,
    /// p2p address
    pub p2p_address: String,
    /// Key pair of peer
    pub key_pair: KeyPair,
}

impl std::cmp::PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::cmp::Eq for Peer {}

pub const CONFIGURATION_PATH: &str = "tests/test_config.json";
pub const CLIENT_CONFIGURATION_PATH: &str = "tests/test_client_config.json";
pub const GENESIS_PATH: &str = "tests/genesis.json";

impl Network {
    /// Starts network with peers with default configuration and specified options.
    /// Returns its info and client for connecting to it.
    pub fn start_test(n_peers: u32, maximum_transactions_in_block: u32) -> (Self, Client) {
        Self::start_test_with_offline(n_peers, maximum_transactions_in_block, 0)
    }

    /// Starts network with peers with default configuration and specified options.
    /// Returns its info and client for connecting to it.
    pub fn start_test_with_offline_and_set_max_faults(
        n_peers: u32,
        maximum_transactions_in_block: u32,
        offline_peers: u32,
        max_faulty_peers: u32,
    ) -> (Self, Client) {
        let mut configuration = Configuration::test();
        configuration
            .queue_configuration
            .maximum_transactions_in_block = maximum_transactions_in_block;
        configuration.sumeragi_configuration.max_faulty_peers = max_faulty_peers;
        let network = Network::new_with_offline_peers(Some(configuration), n_peers, offline_peers)
            .expect("Failed to init peers");
        let client = Client::test(&network.genesis.api_address);
        (network, client)
    }

    /// Starts network with peers with default configuration and specified options.
    /// Returns its info and client for connecting to it.
    pub fn start_test_with_offline(
        n_peers: u32,
        maximum_transactions_in_block: u32,
        offline_peers: u32,
    ) -> (Self, Client) {
        // Calculates max faulty peers based on consensus requirements
        let max_faulty_peers: u32 = (n_peers - 1) / 3;
        Self::start_test_with_offline_and_set_max_faults(
            n_peers,
            maximum_transactions_in_block,
            offline_peers,
            max_faulty_peers,
        )
    }

    /// Adds peer to network
    pub fn add_peer(&self) -> (Peer, Client) {
        let mut client = Client::test(&self.genesis.api_address);
        let peer = Peer::new().expect("Failed to create new peer");
        let mut config = Configuration::test();
        config.sumeragi_configuration.trusted_peers.peers = self.ids().collect();
        config.sumeragi_configuration.max_faulty_peers = (self.peers.len() / 3) as u32;
        peer.start_with_config(config);
        thread::sleep(Configuration::pipeline_time() * 2);
        let add_peer = RegisterBox::new(IdentifiableBox::Peer(
            DataModelPeer::new(peer.id.clone()).into(),
        ));
        client.submit(add_peer).expect("Failed to add new peer.");
        let client = Client::test(&peer.api_address);
        (peer, client)
    }

    /// Creates new network with some ofline peers
    /// # Panics
    /// Panics if fails to find or decode default configuration
    pub fn new_with_offline_peers(
        default_configuration: Option<Configuration>,
        n_peers: u32,
        offline_peers: u32,
    ) -> Result<Self> {
        let n_peers = n_peers - 1;
        let genesis = Peer::new()?;
        let peers = (0..n_peers)
            .map(|_| Peer::new())
            .collect::<Result<Vec<_>>>()?;

        let mut configuration = default_configuration.unwrap_or_else(Configuration::test);
        configuration.genesis_configuration.genesis_block_path = None;
        configuration.sumeragi_configuration.trusted_peers.peers = peers
            .iter()
            .chain(std::iter::once(&genesis))
            .map(|peer| peer.id.clone())
            .collect();

        {
            let mut configuration = configuration.clone();
            configuration.genesis_configuration.genesis_block_path = Some(GENESIS_PATH.to_string());
            drop(genesis.start_with_config(configuration));
        }

        let rng = &mut rand::thread_rng();
        let online_peers = n_peers - offline_peers;

        peers
            .choose_multiple(rng, online_peers as usize)
            .zip(std::iter::repeat_with(|| configuration.clone()))
            .for_each(|(peer, configuration)| {
                drop(peer.start_with_config(configuration));
            });

        thread::sleep(Duration::from_millis(3000) * (n_peers + 1));

        Ok(Self { genesis, peers })
    }

    /// Returns ids of all peers
    pub fn ids(&self) -> impl Iterator<Item = PeerId> + '_ {
        std::iter::once(self.genesis.id.clone())
            .chain(self.peers.iter().map(|peer| peer.id.clone()))
    }

    /// Creates new network from configuration and with that number of peers
    pub fn new(default_configuration: Option<Configuration>, n_peers: u32) -> Result<Self> {
        Self::new_with_offline_peers(default_configuration, n_peers, 0)
    }
}

impl Peer {
    /// Returns per peer config with all addresses, keys, and id setted up
    fn get_config(&self, configuration: Configuration) -> Configuration {
        Configuration {
            sumeragi_configuration: SumeragiConfiguration {
                key_pair: self.key_pair.clone(),
                peer_id: self.id.clone(),
                ..configuration.sumeragi_configuration
            },
            torii_configuration: ToriiConfiguration {
                torii_p2p_url: self.p2p_address.clone(),
                torii_api_url: self.api_address.clone(),
                ..configuration.torii_configuration
            },
            logger_configuration: LoggerConfiguration {
                #[cfg(profile = "bench")]
                max_log_level: LevelEnv::ERROR,
                #[cfg(not(profile = "bench"))]
                max_log_level: LevelEnv::INFO,
                compact_mode: false,
                ..LoggerConfiguration::default()
            },
            public_key: self.key_pair.public_key.clone(),
            private_key: self.key_pair.private_key.clone(),
            ..configuration
        }
    }

    /// Starts peer with config, permissions and temporary directory
    pub fn start_with_config_permissions_dir(
        &self,
        configuration: Configuration,
        permissions: impl Into<PermissionsValidatorBox> + Send + 'static,
        temp_dir: &TempDir,
    ) -> Sender<ShutdownRuntime> {
        let mut configuration = self.get_config(configuration);
        configuration
            .kura_configuration
            .kura_block_store_path(temp_dir.path())
            .unwrap();
        let info_span = iroha_logger::info_span!(
            "test-peer",
            "p2p - {}, api - {}",
            &self.p2p_address,
            &self.api_address
        );
        let (shutdown_runtime_sender, shutdown_runtime_receiver) = mpsc::channel();
        let _join_handle = thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.spawn(
                async move {
                    let iroha = Iroha::new(&configuration, permissions.into()).unwrap();
                    iroha.start().await.expect("Failed to start Iroha.");
                }
                .instrument(info_span),
            );
            if shutdown_runtime_receiver.recv().is_err() {
                // If shutdown sender was dropped - continue running until tests shutdown.
                loop {
                    thread::yield_now();
                }
            }
        });

        thread::sleep(Duration::from_millis(1000));
        shutdown_runtime_sender
    }

    /// Starts peer with config and permissions
    pub fn start_with_config_permissions(
        &self,
        configuration: Configuration,
        permissions: impl Into<PermissionsValidatorBox> + Send + 'static,
    ) -> Sender<ShutdownRuntime> {
        let temp_dir = TempDir::new().expect("Failed to create temp dir.");
        let mut configuration = self.get_config(configuration);
        configuration
            .kura_configuration
            .kura_block_store_path(temp_dir.path())
            .unwrap();
        let info_span = iroha_logger::info_span!(
            "test-peer",
            "p2p - {}, api - {}",
            &self.p2p_address,
            &self.api_address
        );
        let (shutdown_runtime_sender, shutdown_runtime_receiver) = mpsc::channel();
        let _join_handle = thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.spawn(
                async move {
                    let _temp_dir = temp_dir;
                    let iroha = Iroha::new(&configuration, permissions.into()).unwrap();
                    iroha.start().await.expect("Failed to start Iroha.");
                }
                .instrument(info_span),
            );
            if shutdown_runtime_receiver.recv().is_err() {
                // If shutdown sender was dropped - continue running until tests shutdown.
                loop {
                    thread::yield_now();
                }
            }
        });

        thread::sleep(Duration::from_millis(1000));
        shutdown_runtime_sender
    }

    /// Starts peer with config
    pub fn start_with_config(&self, configuration: Configuration) -> Sender<ShutdownRuntime> {
        self.start_with_config_permissions(configuration, AllowAll)
    }

    /// Starts peer
    pub fn start(&self) -> Sender<ShutdownRuntime> {
        self.start_with_config(Configuration::test())
    }

    /// Creates peer
    pub fn new() -> Result<Self> {
        let key_pair = KeyPair::generate()?;
        let p2p_address = format!(
            "127.0.0.1:{}",
            unique_port::get_unique_free_port().map_err(Error::msg)?
        );
        let api_address = format!(
            "127.0.0.1:{}",
            unique_port::get_unique_free_port().map_err(Error::msg)?
        );
        let id = PeerId {
            address: p2p_address.clone(),
            public_key: key_pair.public_key.clone(),
        };
        Ok(Self {
            id,
            key_pair,
            p2p_address,
            api_address,
        })
    }

    /// Starts peer with default configuration.
    /// Returns its info and client for connecting to it.
    pub fn start_test() -> (Self, Client) {
        Self::start_test_with_permissions(AllowAll.into())
    }

    /// Starts peer with default configuration and specified permissions.
    /// Returns its info and client for connecting to it.
    pub fn start_test_with_permissions(permissions: PermissionsValidatorBox) -> (Self, Client) {
        let mut configuration = Configuration::test();
        let peer = Self::new().expect("Failed to create peer.");
        configuration.sumeragi_configuration.trusted_peers.peers =
            std::iter::once(peer.id.clone()).collect();
        configuration.genesis_configuration.genesis_block_path = Some(GENESIS_PATH.to_owned());
        peer.start_with_config_permissions(configuration.clone(), permissions);
        let client = Client::test(&peer.api_address);
        thread::sleep(Duration::from_millis(
            configuration.sumeragi_configuration.pipeline_time_ms(),
        ));
        (peer, client)
    }
}

pub trait TestConfiguration {
    /// Creates test configuration
    fn test() -> Self;
    /// Returns default pipeline time.
    fn pipeline_time() -> Duration;
    /// Returns default time between bocksync gossips for new blocks.
    fn block_sync_gossip_time() -> Duration;
}

pub trait TestClientConfiguration {
    /// Creates test client configuration
    fn test(api_url: &str) -> Self;
}

pub trait TestClient: Sized {
    /// Creates test client from api url
    fn test(api_url: &str) -> Self;

    /// Creates test client from api url and keypair
    fn test_with_key(api_url: &str, keys: KeyPair) -> Self;

    /// Creates test client from api url, keypair, and account id
    fn test_with_account(api_url: &str, keys: KeyPair, account_id: &AccountId) -> Self;

    /// loops for events with filter and handler function
    fn loop_on_events(self, event_filter: EventFilter, f: impl Fn(Result<Event>));

    /// Submits instruction with polling
    fn submit_till<R>(
        &mut self,
        instruction: impl Into<Instruction> + Debug,
        request: R,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug;

    /// Submits instructions with polling
    fn submit_all_till<R>(
        &mut self,
        instructions: Vec<Instruction>,
        request: R,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug;

    /// Polls request till predicate `f` is satisfied, with default period and max attempts.
    fn poll_request<R>(&mut self, request: R, f: impl Fn(&R::Output) -> bool) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug;

    /// Polls request till predicate `f` is satisfied with `period` and `max_attempts` supplied.
    fn poll_request_with_period<R>(
        &mut self,
        request: R,
        period: Duration,
        max_attempts: u32,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug;
}

pub trait TestQueryResult {
    /// Tries to find asset by id
    fn find_asset_by_id(&self, asset_id: &AssetDefinitionId) -> Option<&Asset>;
}

impl TestConfiguration for Configuration {
    fn test() -> Self {
        let mut configuration =
            Self::from_path(CONFIGURATION_PATH).expect("Failed to load configuration.");
        configuration
            .load_environment()
            .expect("Failed to load configuration from environment");
        configuration
    }

    fn pipeline_time() -> Duration {
        Duration::from_millis(Self::test().sumeragi_configuration.pipeline_time_ms())
    }

    fn block_sync_gossip_time() -> Duration {
        Duration::from_millis(Self::test().block_sync_configuration.gossip_period_ms)
    }
}

impl TestClientConfiguration for ClientConfiguration {
    fn test(api_url: &str) -> Self {
        let mut configuration = ClientConfiguration::from_path(CLIENT_CONFIGURATION_PATH)
            .expect("Failed to load configuration.");
        configuration.torii_api_url = api_url.to_owned();
        configuration
    }
}

impl TestClient for Client {
    fn test(api_url: &str) -> Self {
        Client::new(&ClientConfiguration::test(api_url))
    }

    fn test_with_key(api_url: &str, keys: KeyPair) -> Self {
        let mut configuration = ClientConfiguration::test(api_url);
        configuration.public_key = keys.public_key;
        configuration.private_key = keys.private_key;
        Client::new(&configuration)
    }

    fn test_with_account(api_url: &str, keys: KeyPair, account_id: &AccountId) -> Self {
        let mut configuration = ClientConfiguration::test(api_url);
        configuration.account_id = account_id.clone();
        configuration.public_key = keys.public_key;
        configuration.private_key = keys.private_key;
        Client::new(&configuration)
    }

    fn loop_on_events(mut self, event_filter: EventFilter, f: impl Fn(Result<Event>)) {
        for event in self
            .listen_for_events(event_filter)
            .expect("Failed to create event iterator.")
        {
            f(event)
        }
    }

    fn submit_till<R>(
        &mut self,
        instruction: impl Into<Instruction> + Debug,
        request: R,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug,
    {
        self.submit(instruction)
            .expect("Failed to submit instruction.");
        self.poll_request(request, f)
    }

    fn submit_all_till<R>(
        &mut self,
        instructions: Vec<Instruction>,
        request: R,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug,
    {
        self.submit_all(instructions)
            .expect("Failed to submit instruction.");
        self.poll_request(request, f)
    }

    fn poll_request_with_period<R>(
        &mut self,
        request: R,
        period: Duration,
        max_attempts: u32,
        f: impl Fn(&R::Output) -> bool,
    ) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug,
    {
        let mut query_result = None;
        for _ in 0..max_attempts {
            thread::sleep(period);
            query_result = match self.request(request.clone()) {
                Ok(result) if f(&result) => return result,
                result => Some(result),
            }
        }
        panic!("Failed to wait for query request completion that would satisfy specified closure. Got this query result instead: {:?}", &query_result)
    }

    fn poll_request<R>(&mut self, request: R, f: impl Fn(&R::Output) -> bool) -> R::Output
    where
        R: Query + Into<QueryRequest> + Debug + Clone,
        <R::Output as TryFrom<Value>>::Error: Into<iroha_error::Error>,
        R::Output: Clone + Debug,
    {
        self.poll_request_with_period(request, Configuration::pipeline_time(), 10, f)
    }
}

impl TestQueryResult for QueryResult {
    fn find_asset_by_id(&self, asset_id: &AssetDefinitionId) -> Option<&Asset> {
        let assets = if let QueryResult(Value::Vec(assets)) = self {
            assets
        } else {
            panic!("Wrong Query Result Type.");
        };
        assets.iter().find_map(|asset| {
            if let Value::Identifiable(IdentifiableBox::Asset(asset)) = asset {
                if &asset.id.definition_id == asset_id {
                    return Some(asset.as_ref());
                }
            }
            None
        })
    }
}