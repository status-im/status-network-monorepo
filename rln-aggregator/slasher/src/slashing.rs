use tokio::sync::mpsc::Receiver;
use tracing::warn;
use crate::prover_proto::RlnAggProof;

pub(crate) struct SlashingService {
    slashing_rx: Receiver<(RlnAggProof, RlnAggProof)>,
}

impl SlashingService {
    
    pub(crate) fn new(slashing_rx: Receiver<(RlnAggProof, RlnAggProof)>) -> Self {
        Self {
            slashing_rx
        }
    }
    
    #[tracing::instrument(skip(self))]
    pub(crate) async fn serve(&mut self) -> anyhow::Result<()> {
        loop {
            let res = self.slashing_rx.recv().await;
        
            if let Some((_proof_1, _proof_2)) = res {
                todo!()
            } else {
                warn!("Slashing channel has been closed");
                break;
            }
        }
        
        Ok(())
    }
}
