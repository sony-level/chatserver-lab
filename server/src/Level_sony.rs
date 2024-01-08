use anyhow::Ok;
use async_std::sync::RwLock;
use async_trait::async_trait;
use std::{collections::{HashMap, HashSet, VecDeque}, clone};
use uuid::Uuid;

use crate::{
  core::{MessageServer, MAILBOX_SIZE, WORKPROOF_STRENGTH},
  messages::{
    ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply, FullyQualifiedMessage,
    Sequence, ServerId,
  },
  workproof::verify_workproof,
};

#[cfg(feature = "federation")]
use crate::messages::{Outgoing, ServerMessage, ServerReply};
use crate::netproto::decode::u128;
use crate::workproof::gen_workproof;

#[derive(Clone)]
struct ClientInfo {
  name: String,
  seq: u128,
  messages: VecDeque<(ClientId, String)>,
  delayed_messages: VecDeque<(ClientId, String)>,
  mail_box: Vec<ClientMessage>
}

pub struct Server {
  id : ServerId,
  clients:HashMap<ClientId, ClientInfo>,
  routes: HashMap<ServerId, Vec<ServerId>>,
  client_to_server: HashMap<ClientId, ServerId>,
}

#[async_trait]
impl MessageServer for Server {
  const GROUP_NAME: &'static str = "level sony";

  fn new(id: ServerId) -> Self {
    let server = Server {
      id,
      clients: HashMap::new(),
      routes:HashMap::new(),
      client_to_server: HashMap::new(),
    };
    server
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  async fn register_local_client(&self, name: String) -> ClientId {
    let uuid= Uuid::new_v4();
    let clientid=ClientId(uuid);
    clientid
  }

  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,) -> Result<A, ClientError> {
    //let clientid = sequence.src;
    let mut case = 0;

    if gen_workproof(sequence.workproof, WORKPROOF_STRENGTH, u128::MAX).is_none() {
        case = 1;
    }

    if !self.clients.contains_key(&sequence.src) {
        case = 2;
    } else if let Some(clientinfo) = self.clients.get(&sequence.src) {
        if clientinfo.seq != sequence.seqid {
            case = 3;
        }
    }

    if case == 0 {
        // Check mailbox size only if the other checks passed
        if let Some(clientinfo) = self.clients.get(&sequence.src) {
            if clientinfo.mail_box.len() >= MAILBOX_SIZE {
                case = 4;
            }
        }
    }

    match case {
        0 => Err(ClientError::InternalError),
        1 => Err(ClientError::WorkProofError),
        2 => Err(ClientError::UnknownClient),
        3 => Err(ClientError::SequenceError),
        4 => Err(ClientError::BoxFull(sequence.src)),
        _ => Err(ClientError::InternalError),
    }
}


    async fn handle_client_message(&self, src: ClientId, msg: ClientMessage) -> Vec<ClientReply> {
      let mut replies = Vec::new();

        match msg {
            ClientMessage::Text { dest, content } => {
                if let Some(dest_info) = self.clients.get(&dest) {
                    if dest_info.mail_box.len() < MAILBOX_SIZE {
                        // Ajout message dans la boite du client !!!!!!!!!!!!!!!!!!!;
                        replies.push(ClientReply::Delivered);
                    } else {
                        replies.push(ClientReply::Error(ClientError::BoxFull(dest)));
                    }
                } else {
                    replies.push(ClientReply::Error(ClientError::UnknownClient));
                }
            },
            ClientMessage::MText { dest, content } => {
                for d in dest {
                    if let Some(dest_info) = self.clients.get(&d) {
                        if dest_info.mail_box.len() < MAILBOX_SIZE {
                            // Ajout message dans la boite du client !!!!!!!!!!!!!!!!!!!
                            replies.push(ClientReply::Delivered);
                        } else {
                            replies.push(ClientReply::Error(ClientError::BoxFull(d)));
                        }
                    } else {
                        replies.push(ClientReply::Error(ClientError::UnknownClient));
                    }
                }
            },
        }

        replies
    }

  #[cfg(feature = "federation")]
  async fn handle_server_message(&self, msg: ServerMessage) -> ServerReply {
    match msg {
      ServerMessage::Announce { route, clients } => {
          // Traitement simplifié de l'annonce
          ServerReply::EmptyRoute
      },
      ServerMessage::Message(fqm) => {
          // Traitement simplifié du message
          ServerReply::Outgoing(vec![])
      },
    }
  }

  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    
    if let Some(client_info) = self.clients.get(&client) {
      if let Some((src, content)) = client_info.messages.front() {
          // Cloner le contenu du message pour le renvoyer
          return ClientPollReply::Message { src: *src, content: content.clone() };
      }
      
    }

    // Retourner Nothing si aucun message n'est disponible
    ClientPollReply::Nothing
   
  }

  async fn list_users(&self) -> HashMap<ClientId, String> {
    
    self.clients.iter().map(|(&client_id, client_info)| {
      (client_id, client_info.name.clone())
    }).collect()
    
  }

  #[cfg(feature = "federation")]
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
     
    // Rechercher un chemin vers le serveur de destination
    self.routes.get(&destination).cloned()
   
  }

}

impl Server {
  // write your own methods here
}

#[cfg(test)]
mod test {
  use crate::testing::test_message_server;

  use super::*;

  #[test]
  fn tester() {
    test_message_server::<Server>();
  }
}
