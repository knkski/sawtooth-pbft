/*
 * Copyright 2018 Bitwise IO, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */

//! Entry point for the consensus algorithm, including the main event loop

use std::sync::mpsc::{Receiver, RecvTimeoutError};

use sawtooth_sdk::consensus::{engine::*, service::Service};

use node::PbftNode;
use hex;
use config;
use timing;

use error::PbftError;

#[derive(Default)]
pub struct PbftEngine {}

impl PbftEngine {
    pub fn new() -> Self {
        PbftEngine {}
    }
}

impl Engine for PbftEngine {
    fn start(
        &mut self,
        updates: Receiver<Update>,
        mut service: Box<Service>,
        startup_state: StartupState,
    ) {
        let StartupState {
            chain_head,
            peers,
            local_peer_info,
        } = startup_state;

        let mut deduped_peers = vec![];
        for peer_info in peers.into_iter() {
            if !deduped_peers.contains(&peer_info.peer_id) {
                deduped_peers.push(peer_info.peer_id);
            }
        }

        info!(
            "Starting node {} with peers: {:#?}",
            &hex::encode(&local_peer_info.peer_id)[..6],
            deduped_peers
        );

        // Load on-chain settings
        let config = config::load_pbft_config(chain_head.block_id, &mut *service);

        let tmp_node_id = deduped_peers.len() as u64;
        deduped_peers.push(local_peer_info.peer_id);

        let mut working_ticker = timing::Ticker::new(config.block_duration);
        let mut backlog_ticker = timing::Ticker::new(config.message_timeout);

        let mut node = PbftNode::new(tmp_node_id, &config, deduped_peers, service);

        debug!("Starting state: {:#?}", node.state);

        // Event loop. Keep going until we receive a shutdown message.
        loop {
            let incoming_message = updates.recv_timeout(config.message_timeout);

            let res = match incoming_message {
                Ok(Update::BlockNew(block)) => node.on_block_new(block),
                Ok(Update::BlockValid(block_id)) => node.on_block_valid(block_id),
                Ok(Update::BlockInvalid(_)) => {
                    warn!(
                        "{}: BlockInvalid received, starting view change",
                        node.state
                    );
                    node.start_view_change()
                }
                Ok(Update::BlockCommit(block_id)) => node.on_block_commit(block_id),
                Ok(Update::PeerMessage(message, _sender_id)) => node.on_peer_message(&message),
                Ok(Update::Shutdown) => break,
                Ok(Update::PeerConnected(peer_info)) => {
                    node.on_peer_change(peer_info.peer_id, true)
                }
                Ok(Update::PeerDisconnected(peer_id)) => node.on_peer_change(peer_id, false),
                Err(RecvTimeoutError::Timeout) => Err(PbftError::Timeout),
                Err(RecvTimeoutError::Disconnected) => {
                    error!("Disconnected from validator");
                    break;
                }
            };
            handle_pbft_result(res);

            working_ticker.tick(|| {
                if let Err(e) = node.try_publish() {
                    error!("{}", e);
                }

                // Every so often, check to see if timeout has expired; initiate ViewChange if necessary
                if node.check_timeout_expired() {
                    handle_pbft_result(node.start_view_change());
                }
            });

            backlog_ticker.tick(|| {
                handle_pbft_result(node.retry_backlog());
            })
        }
    }

    fn version(&self) -> String {
        String::from(env!("CARGO_PKG_VERSION"))
    }

    fn name(&self) -> String {
        String::from(env!("CARGO_PKG_NAME"))
    }
}

fn handle_pbft_result(res: Result<(), PbftError>) {
    if let Err(e) = res {
        match e {
            PbftError::Timeout => (),
            PbftError::WrongNumMessages(_, _, _) | PbftError::NotReadyForMessage => trace!("{}", e),
            _ => error!("{}", e),
        }
    }
}
