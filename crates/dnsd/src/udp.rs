use std::net::Ipv4Addr;

use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use hickory_proto::serialize::binary::BinEncoder;

use common::{AppError, AppResult};

/// Handle a single DNS packet, returning a response buffer.
#[tracing::instrument(level = "trace", skip(packet))]
pub fn handle_packet(packet: &[u8]) -> AppResult<Vec<u8>> {
    if packet.len() < 12 {
        return Err(AppError::Message("packet too short".into()));
    }

    // Parse header fields we care about.
    let id = u16::from_be_bytes([packet[0], packet[1]]);
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let rd = flags & 0x0100 != 0;

    // Build a minimal response using hickory-proto.
    let mut message = Message::new();
    message.set_id(id);
    message.set_message_type(MessageType::Response);
    message.set_op_code(OpCode::Query);
    message.set_recursion_desired(rd);
    message.set_recursion_available(true);
    message.set_response_code(ResponseCode::NoError);

    let name = Name::from_ascii("test.fdns.run.").expect("static name");
    message.add_query(hickory_proto::op::Query::query(name.clone(), RecordType::A));

    let record = Record::from_rdata(name, 60, RData::A(Ipv4Addr::new(127, 0, 0, 1)));
    message.add_answer(record);

    let mut out = Vec::with_capacity(512);
    let mut encoder = BinEncoder::new(&mut out);
    message
        .emit(&mut encoder)
        .map_err(|e| AppError::Message(e.to_string()))?;
    Ok(out)
}
