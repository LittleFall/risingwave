use std::collections::HashMap;

use futures::channel::mpsc::UnboundedSender;
use risingwave_common::error::Result;

use crate::executor::*;

/// [`BarrierManager`] manages barrier control flow, used by local stream manager.
pub struct BarrierManager {
    /// Stores all materialized view source sender.
    sender_placeholder: HashMap<u32, UnboundedSender<Message>>,
}

impl Default for BarrierManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BarrierManager {
    pub fn new() -> Self {
        Self {
            sender_placeholder: HashMap::new(),
        }
    }
    /// register sender for materialized view, used to send barriers.
    pub fn register_sender(&mut self, actor_id: u32, sender: UnboundedSender<Message>) {
        debug!("register sender: {}", actor_id);
        self.sender_placeholder.insert(actor_id, sender);
    }

    /// broadcast a barrier to all senders with specific epoch.
    /// TODO: async collect barrier flush state from hummock.
    pub fn send_barrier(&mut self, barrier: &Barrier) -> Result<()> {
        for sender in self.sender_placeholder.values() {
            sender
                .unbounded_send(Message::Barrier(barrier.clone()))
                .unwrap();
        }

        if let Mutation::Stop(actors) = barrier.mutation.clone() {
            actors.iter().for_each(|actor| {
                if let Some(sender) = self.sender_placeholder.remove(actor) {
                    sender.close_channel();
                }
            });
        }

        Ok(())
    }
}