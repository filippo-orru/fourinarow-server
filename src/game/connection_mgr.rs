use actix::{Actor, Addr, Context, Handler, Message};

use super::client_state::{ClientState, ClientStateMessage};

pub struct ConnectionManager {
    connections: Vec<Connection>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager {
            connections: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct Connection {
    addr: Addr<ClientState>,
}

pub enum ConnectionManagerMsg {
    Hello(Addr<ClientState>), // sent when client first connects
    Bye(Addr<ClientState>),   // sent when client disconnects
}

impl Message for ConnectionManagerMsg {
    type Result = Result<(), ()>;
}

impl Handler<ConnectionManagerMsg> for ConnectionManager {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ConnectionManagerMsg, ctx: &mut Self::Context) -> Self::Result {
        use ConnectionManagerMsg::*;
        match msg {
            Hello(client_state_addr) => client_state_addr.do_send(
                ClientStateMessage::CurrentServerState(self.connections.len(), false),
            ),
            Bye(client_state_addr) => {
                if let Some(index) = self
                    .connections
                    .iter()
                    .position(|conn| conn.addr == client_state_addr)
                {
                    self.connections.remove(index);
                }
            }
        }
        Ok(())
    }
}

impl Actor for ConnectionManager {
    type Context = Context<Self>;
}
