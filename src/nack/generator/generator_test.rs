use super::*;
use crate::mock::mock_stream::MockStream;
use crate::stream_info::RTCPFeedback;

use rtcp::transport_feedbacks::transport_layer_nack::TransportLayerNack;

#[tokio::test]
async fn test_generator_interceptor() -> Result<()> {
    const INTERVAL: Duration = Duration::from_millis(10);
    let icpr: Arc<dyn Interceptor + Send + Sync> = Arc::new(
        Generator::builder()
            .with_log2_size_minus_6(0)
            .with_skip_last_n(2)
            .with_interval(INTERVAL)
            .build(),
    );

    let stream = MockStream::new(
        &StreamInfo {
            ssrc: 1,
            rtcp_feedback: vec![RTCPFeedback {
                typ: "nack".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        },
        icpr,
    )
    .await;

    for seq_num in [10, 11, 12, 14, 16, 18] {
        stream
            .receive_rtp(rtp::packet::Packet {
                header: rtp::header::Header {
                    sequence_number: seq_num,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await;

        if let Some(r) = tokio::time::timeout(Duration::from_millis(10), stream.read_rtp()).await? {
            if let Ok(r) = r {
                assert_eq!(seq_num, r.header.sequence_number);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    tokio::time::sleep(INTERVAL * 2).await; // wait for at least 2 nack packets

    // ignore the first nack, it might only contain the sequence id 13 as missing
    let _ = stream.written_rtcp().await;

    if let Some(r) = tokio::time::timeout(Duration::from_millis(10), stream.written_rtcp()).await? {
        if let Some(p) = r.as_any().downcast_ref::<TransportLayerNack>() {
            assert_eq!(13, p.nacks[0].packet_id);
            assert_eq!(0b10, p.nacks[0].lost_packets); // we want packets: 13, 15 (not packet 17, because skipLastN is setReceived to 2)
        } else {
            assert!(false, "single packet RTCP Compound Packet expected");
        }
    } else {
        assert!(false);
    }

    stream.close().await?;

    Ok(())
}
