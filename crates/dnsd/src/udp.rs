use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::{RData, Record, RecordType, rdata};
use hickory_proto::serialize::binary::{BinEncodable, BinEncoder};

use crate::redis_cache::{self, CacheError};
use crate::sign;
use common::{AppError, AppResult};

/// Handle a single DNS packet, returning a response buffer.
///
/// The Redis `pool` is consulted for a matching IPv4 address based on the
/// leading label of the query name. If the key is missing, the response will be
/// an NXDOMAIN.
#[tracing::instrument(level = "trace", skip(packet, pool))]
pub async fn handle_packet(packet: &[u8], pool: &redis_cache::RedisPool) -> AppResult<Vec<u8>> {
    if packet.len() < 12 {
        return Err(AppError::Message("packet too short".into()));
    }

    // Parse header fields we care about.
    let id = u16::from_be_bytes([packet[0], packet[1]]);
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let rd = flags & 0x0100 != 0;

    // Parse the query name to determine the lookup key.
    let req = Message::from_vec(packet).map_err(|e| AppError::Message(e.to_string()))?;
    let query = req
        .query()
        .ok_or_else(|| AppError::Message("no query".into()))?;
    let qname = query.name();
    let label = qname
        .iter()
        .next()
        .and_then(|l| std::str::from_utf8(l).ok())
        .ok_or_else(|| AppError::Message("invalid label".into()))?;

    // Look up the IPv4 address in Redis.
    let lookup = match redis_cache::get_slot(pool, label).await {
        Ok(ip) => Some(ip),
        Err(CacheError::NXDomain) => None,
        Err(e) => return Err(AppError::Message(e.to_string())),
    };

    // Build the DNS response.
    let mut message = Message::new();
    message.set_id(id);
    message.set_message_type(MessageType::Response);
    message.set_op_code(OpCode::Query);
    message.set_recursion_desired(rd);
    message.set_recursion_available(true);

    message.add_query(query.clone());

    if let Some(ip) = lookup {
        message.set_response_code(ResponseCode::NoError);
        let record = Record::from_rdata(qname.clone(), 60, RData::A(rdata::A(ip)));
        message.add_answer(record);

        if let Some(signer) = sign::signer() {
            let mut rrset = Vec::new();
            {
                let mut enc = BinEncoder::new(&mut rrset);
                for rec in message.answers() {
                    rec.emit(&mut enc)
                        .map_err(|e| AppError::Message(e.to_string()))?;
                }
            }
            let sig = signer.rrsig_record(qname, RecordType::A, 60, &rrset);
            message.add_answer(sig);
        }
    } else {
        message.set_response_code(ResponseCode::NXDomain);
    }

    let mut out = Vec::with_capacity(512);
    let mut encoder = BinEncoder::new(&mut out);
    message
        .emit(&mut encoder)
        .map_err(|e| AppError::Message(e.to_string()))?;
    Ok(out)
}
