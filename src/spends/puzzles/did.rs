mod create_did;
mod did_info;
mod did_spend;

pub use create_did::*;
pub use did_info::*;
pub use did_spend::*;

#[cfg(test)]
mod tests {
    use super::*;

    use chia_bls::{sign, Signature};
    use chia_protocol::SpendBundle;
    use chia_puzzles::{
        standard::{StandardArgs, STANDARD_PUZZLE_HASH},
        DeriveSynthetic,
    };
    use clvm_utils::{CurriedProgram, ToTreeHash};
    use clvmr::Allocator;

    use crate::{
        testing::SECRET_KEY, Chainable, Launcher, RequiredSignature, SpendContext, StandardSpend,
        WalletSimulator,
    };

    #[tokio::test]
    async fn test_create_did() -> anyhow::Result<()> {
        let sim = WalletSimulator::new().await;
        let peer = sim.peer().await;

        let sk = SECRET_KEY.derive_synthetic();
        let pk = sk.public_key();

        let puzzle_hash = CurriedProgram {
            program: STANDARD_PUZZLE_HASH,
            args: StandardArgs { synthetic_key: pk },
        }
        .tree_hash()
        .into();

        let parent = sim.generate_coin(puzzle_hash, 1).await.coin;

        let mut allocator = Allocator::new();
        let mut ctx = SpendContext::new(&mut allocator);

        let (launch_singleton, _did_info) = Launcher::new(parent.coin_id(), 1)
            .create(&mut ctx)?
            .create_standard_did(&mut ctx, pk)?;

        StandardSpend::new()
            .chain(launch_singleton)
            .finish(&mut ctx, parent, pk)?;

        let coin_spends = ctx.take_spends();

        let mut spend_bundle = SpendBundle::new(coin_spends, Signature::default());

        let required_signatures = RequiredSignature::from_coin_spends(
            &mut allocator,
            &spend_bundle.coin_spends,
            WalletSimulator::AGG_SIG_ME.into(),
        )
        .unwrap();

        for required in required_signatures {
            spend_bundle.aggregated_signature += &sign(&sk, required.final_message());
        }

        let ack = peer.send_transaction(spend_bundle).await.unwrap();
        assert_eq!(ack.error, None);
        assert_eq!(ack.status, 1);

        // Make sure the DID was created.
        let found_coins = peer
            .register_for_ph_updates(vec![puzzle_hash], 0)
            .await
            .unwrap();
        assert_eq!(found_coins.len(), 2);

        Ok(())
    }
}