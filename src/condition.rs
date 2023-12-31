use chia_bls::PublicKey;
use chia_protocol::{Bytes, Bytes32};
use clvm_traits::{
    clvm_list, destructure_list, from_clvm, match_list, to_clvm, FromClvm, MatchByte, ToClvm,
};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
#[repr(u64)]
pub enum Condition<T>
where
    T: Clone,
{
    Remark = 1,

    AggSigParent {
        public_key: PublicKey,
        message: Bytes,
    } = 43,

    AggSigPuzzle {
        public_key: PublicKey,
        message: Bytes,
    } = 44,

    AggSigAmount {
        public_key: PublicKey,
        message: Bytes,
    } = 45,

    AggSigPuzzleAmount {
        public_key: PublicKey,
        message: Bytes,
    } = 46,

    AggSigParentAmount {
        public_key: PublicKey,
        message: Bytes,
    } = 47,

    AggSigParentPuzzle {
        public_key: PublicKey,
        message: Bytes,
    } = 48,

    AggSigUnsafe {
        public_key: PublicKey,
        message: Bytes,
    } = 49,

    AggSigMe {
        public_key: PublicKey,
        message: Bytes,
    } = 50,

    #[clvm(tuple)]
    CreateCoin(CreateCoin) = 51,

    ReserveFee {
        amount: u64,
    } = 52,

    CreateCoinAnnouncement {
        message: Bytes,
    } = 60,

    AssertCoinAnnouncement {
        announcement_id: Bytes,
    } = 61,

    CreatePuzzleAnnouncement {
        message: Bytes,
    } = 62,

    AssertPuzzleAnnouncement {
        announcement_id: Bytes,
    } = 63,

    AssertConcurrentSpend {
        coin_id: Bytes32,
    } = 64,

    AssertConcurrentPuzzle {
        puzzle_hash: Bytes32,
    } = 65,

    AssertMyCoinId {
        coin_id: Bytes32,
    } = 70,

    AssertMyParentId {
        parent_id: Bytes32,
    } = 71,

    AssertMyPuzzleHash {
        puzzle_hash: Bytes32,
    } = 72,

    AssertMyAmount {
        amount: u64,
    } = 73,

    AssertMyBirthSeconds {
        seconds: u64,
    } = 74,

    AssertMyBirthHeight {
        block_height: u32,
    } = 75,

    AssertEphemeral = 76,

    AssertSecondsRelative {
        seconds: u64,
    } = 80,

    AssertSecondsAbsolute {
        seconds: u64,
    } = 81,

    AssertHeightRelative {
        block_height: u32,
    } = 82,

    AssertHeightAbsolute {
        block_height: u32,
    } = 83,

    AssertBeforeSecondsRelative {
        seconds: u64,
    } = 84,

    AssertBeforeSecondsAbsolute {
        seconds: u64,
    } = 85,

    AssertBeforeHeightRelative {
        block_height: u32,
    } = 86,

    AssertBeforeHeightAbsolute {
        block_height: u32,
    } = 87,

    #[clvm(tuple)]
    Softfork {
        cost: u64,
        rest: T,
    } = 90,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(raw, list)]
pub enum CreateCoin {
    Normal {
        puzzle_hash: Bytes32,
        amount: u64,
    },
    Memos {
        puzzle_hash: Bytes32,
        amount: u64,
        memos: Vec<Bytes32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(raw, tuple)]
pub enum CatCondition<T>
where
    T: Clone,
{
    Normal(Condition<T>),
    RunTail(RunTail<T>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTail<T> {
    pub program: T,
    pub solution: T,
}

impl<Node, T> ToClvm<Node> for RunTail<T>
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, {
        clvm_list!(51, (), -113, &self.program, &self.solution).to_clvm(f)
    });
}

impl<Node, T> FromClvm<Node> for RunTail<T>
where
    Node: Clone,
    T: FromClvm<Node>,
{
    from_clvm!(Node, f, ptr, {
        let destructure_list!(_, _, _, program, solution) =
            <match_list!(MatchByte::<51>, (), MatchByte::<142>, T, T)>::from_clvm(f, ptr)?;
        Ok(Self { program, solution })
    });
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use clvm_traits::{FromPtr, ToPtr};
    use clvmr::{allocator::NodePtr, serde::node_to_bytes, Allocator};
    use hex_literal::hex;

    use super::*;

    fn check<T>(value: T, expected: &[u8])
    where
        T: ToPtr + FromPtr + PartialEq + Debug,
    {
        let a = &mut Allocator::new();
        let serialized = value.to_ptr(a).unwrap();
        let deserialized = T::from_ptr(a, serialized).unwrap();
        assert_eq!(value, deserialized);

        let bytes = node_to_bytes(a, serialized).unwrap();
        assert_eq!(hex::encode(bytes), hex::encode(expected));
    }

    #[test]
    fn test() {
        check(
            Condition::<NodePtr>::CreateCoin(CreateCoin::Memos {
                puzzle_hash: Bytes32::from([0; 32]),
                amount: 0,
                memos: vec![Bytes32::from([1; 32])],
            }),
            &hex!(
                "
                ff33ffa00000000000000000000000000000000000000000000000000000000000000000ff8
                0ffffa001010101010101010101010101010101010101010101010101010101010101018080
                "
            ),
        );

        check(
            Condition::<NodePtr>::CreateCoin(CreateCoin::Normal {
                puzzle_hash: Bytes32::from([0; 32]),
                amount: 0,
            }),
            &hex!(
                "
                ff33ffa00000000000000000000000000000000000000000000000000000000000000000ff8080
                "
            ),
        );
    }
}
