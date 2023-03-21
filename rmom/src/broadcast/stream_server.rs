use async_broadcast::Receiver;
use futures_lite::stream::{Filter, Stream, StreamExt};
use log::info;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tonic::{transport::Server, Code, Request, Response, Status};
use uuid::Uuid;

use super::queue::{BroadcastEnd, ChannelId, ChannelReceiver};
use crate::messages::message_stream_server::{MessageStream, MessageStreamServer};
use crate::messages::{Message, Push, PushOkResponse, SubscriptionRequest};

pub type ChannelStream = Pin<Box<dyn Stream<Item = Result<Message, Status>> + Send>>;

pub struct StreamServer {
    host: String,
    port: u16,
    channel_receivers: Arc<Mutex<HashMap<ChannelId, ChannelReceiver>>>,
    broadcast_end: Arc<Mutex<BroadcastEnd>>,
}

#[tonic::async_trait]
impl MessageStream for StreamServer {
    type SubscribeToChannelStream = ChannelStream;

    async fn subscribe_to_channel(
        &self,
        req: Request<SubscriptionRequest>,
    ) -> Result<Response<Self::SubscribeToChannelStream>, Status> {
        let req = req.into_inner();
        info!("Request to SUBSCRIBE to channel {0}", req.channel_id);

        if let Ok(chan_id) = Uuid::parse_str(req.channel_id.as_str()) {
            let mut lock = self.channel_receivers.lock().unwrap();
            let chan_receiver = lock.remove(&chan_id);
            drop(lock);

            if let Some(mut chan_receiver) = chan_receiver {
                let stream = chan_receiver.receiver.filter(move |msg| {
                    if let Some(chan_topic) = &chan_receiver.topic {
                        if let Ok(msg) = msg {
                            return chan_topic.as_str() == msg.topic.as_str();
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                });

                Ok(Response::new(Box::pin(stream)))
            } else {
                Err(Status::new(Code::NotFound, "Channel not found"))
            }
        } else {
            Err(Status::new(
                Code::InvalidArgument,
                "Invalid channel_id: not a uuid v4",
            ))
        }
    }

    async fn push_to_channel(
        &self,
        push: Request<Push>,
    ) -> Result<Response<PushOkResponse>, Status> {
        let push = push.into_inner();
        info!("Request to PUSH to channel {0}", push.channel_id);

        if let Ok(chan_id) = Uuid::parse_str(push.channel_id.as_str()) {
            let chan_sender = self.broadcast_end.lock().unwrap();
            let message = Message {
                id: Uuid::new_v4().to_string(),
                content: push.content,
                topic: push.topic,
            };

            match chan_sender.broadcast(message) {
                Ok(()) => {
                    drop(chan_sender);
                    Ok(Response::new(PushOkResponse {}))
                }
                Err(msg) => {
                    drop(chan_sender);
                    Err(Status::new(Code::Internal, msg))
                }
            }
        } else {
            Err(Status::new(
                Code::InvalidArgument,
                "Invalid channel_id: not a uuid v4",
            ))
        }
    }
}

impl StreamServer {
    pub fn new(
        host: String,
        port: u16,
        broadcast_end: Arc<Mutex<BroadcastEnd>>,
        channel_receivers: Arc<Mutex<HashMap<ChannelId, ChannelReceiver>>>,
    ) -> Self {
        StreamServer {
            host,
            port,
            channel_receivers,
            broadcast_end,
        }
    }

    pub async fn run(self) {
        let addr = format!("{}:{}", self.host, self.port);

        println!("Running stream server on {addr}");
        Server::builder()
            .add_service(MessageStreamServer::new(self))
            .serve(addr.to_socket_addrs().unwrap().next().unwrap())
            .await
            .unwrap();
    }
}
