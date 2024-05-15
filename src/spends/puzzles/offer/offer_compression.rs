use std::{
    array::TryFromSliceError,
    io::{self, ErrorKind, Read},
};

use chia_protocol::SpendBundle;
use chia_traits::Streamable;
use chia_puzzles::{
    cat::{CAT_PUZZLE, CAT_PUZZLE_V1},
    nft::{
        NFT_METADATA_UPDATER_PUZZLE, NFT_OWNERSHIP_LAYER_PUZZLE, NFT_ROYALTY_TRANSFER_PUZZLE,
        NFT_STATE_LAYER_PUZZLE,
    },
    offer::{SETTLEMENT_PAYMENTS_PUZZLE, SETTLEMENT_PAYMENTS_PUZZLE_V1},
    singleton::SINGLETON_TOP_LAYER_PUZZLE,
    standard::STANDARD_PUZZLE,
};
use flate2::{
    read::{ZlibDecoder, ZlibEncoder},
    Compress, Compression, Decompress, FlushDecompress,
};
use thiserror::Error;

macro_rules! define_compression_versions {
    ( $( $version:expr => $( $bytes:expr ),+ ; )+ ) => {
        fn zdict_for_version(version: u16) -> Vec<u8> {
            let mut bytes = Vec::new();
            $( if version >= $version {
                $( bytes.extend_from_slice(&$bytes); )+
            } )+
            bytes
        }

        /// Returns the required compression version for the given puzzle reveals.
        pub fn required_compression_version(puzzles: Vec<Vec<u8>>) -> u16 {
            let mut required_version = MIN_VERSION;
            $( {
                $( if required_version < $version && puzzles.iter().any(|puzzle| puzzle == &$bytes) {
                    required_version = $version;
                } )+
            } )+
            required_version
        }
    };
}

const MIN_VERSION: u16 = 6;
const MAX_VERSION: u16 = 6;

define_compression_versions!(
    1 => STANDARD_PUZZLE, CAT_PUZZLE_V1;
    2 => SETTLEMENT_PAYMENTS_PUZZLE_V1;
    3 => SINGLETON_TOP_LAYER_PUZZLE, NFT_STATE_LAYER_PUZZLE,
         NFT_OWNERSHIP_LAYER_PUZZLE, NFT_METADATA_UPDATER_PUZZLE,
         NFT_ROYALTY_TRANSFER_PUZZLE;
    4 => CAT_PUZZLE;
    5 => SETTLEMENT_PAYMENTS_PUZZLE;
    6 => [0; 0]; // Purposefully break backwards compatibility.
);

/// An error than can occur while decompressing an offer.
#[derive(Debug, Error)]
pub enum DecompressionError {
    /// An io error.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// An error that occurred while trying to convert a slice to an array.
    #[error("{0}")]
    TryFromSlice(#[from] TryFromSliceError),

    /// The input is missing the version prefix.
    #[error("missing version prefix")]
    MissingVersionPrefix,

    /// The version is unsupported.
    #[error("unsupported version")]
    UnsupportedVersion,

    /// A streamable error.
    #[error("streamable error: {0}")]
    Streamable(#[from] chia_traits::Error),
}

/// Decompresses an offer spend bundle.
pub fn decompress_offer(bytes: &[u8]) -> Result<SpendBundle, DecompressionError> {
    let decompressed_bytes = decompress_offer_bytes(bytes)?;
    Ok(SpendBundle::from_bytes(&decompressed_bytes)?)
}

/// Decompresses an offer spend bundle into bytes.
pub fn decompress_offer_bytes(bytes: &[u8]) -> Result<Vec<u8>, DecompressionError> {
    let version_bytes: [u8; 2] = bytes
        .get(0..2)
        .ok_or(DecompressionError::MissingVersionPrefix)?
        .try_into()?;

    let version = u16::from_be_bytes(version_bytes);

    if version > MAX_VERSION {
        return Err(DecompressionError::UnsupportedVersion);
    }

    let zdict = zdict_for_version(version);

    Ok(decompress(&bytes[2..], &zdict)?)
}

#[derive(Debug, Error)]
pub enum CompressionError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("streamable error: {0}")]
    Streamable(#[from] chia_traits::Error),
}

/// Compresses an offer spend bundle.
pub fn compress_offer(spend_bundle: SpendBundle) -> Result<Vec<u8>, CompressionError> {
    let bytes = spend_bundle.to_bytes()?;
    let version = required_compression_version(
        spend_bundle
            .coin_spends
            .into_iter()
            .map(|cs| cs.puzzle_reveal.to_vec())
            .collect(),
    );
    Ok(compress_offer_bytes(&bytes, version)?)
}

/// Compresses an offer spend bundle from bytes.
pub fn compress_offer_bytes(bytes: &[u8], version: u16) -> io::Result<Vec<u8>> {
    let mut output = version.to_be_bytes().to_vec();
    let zdict = zdict_for_version(version);
    output.extend(compress(bytes, &zdict)?);
    Ok(output)
}

fn decompress(input: &[u8], zdict: &[u8]) -> io::Result<Vec<u8>> {
    let mut decompress = Decompress::new(true);
    if decompress
        .decompress(input, &mut [], FlushDecompress::Finish)
        .is_ok()
    {
        return Err(io::Error::new(
            ErrorKind::Unsupported,
            "cannot decompress uncompressed input",
        ));
    }
    decompress.set_dictionary(zdict)?;
    let i = decompress.total_in();
    let mut decoder = ZlibDecoder::new_with_decompress(&input[i as usize..], decompress);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn compress(input: &[u8], zdict: &[u8]) -> io::Result<Vec<u8>> {
    let mut compress = Compress::new(Compression::new(6), true);
    compress.set_dictionary(zdict)?;
    let mut encoder = ZlibEncoder::new_with_compress(input, compress);
    let mut output = Vec::new();
    encoder.read_to_end(&mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use chia_protocol::SpendBundle;
    use chia_traits::Streamable;
    use hex::ToHex;
    use hex_literal::hex;

    use super::*;

    #[test]
    fn test_compression() {
        for version in MIN_VERSION..=MAX_VERSION {
            let output = compress_offer_bytes(&DECOMPRESSED_OFFER_HEX, version).unwrap();

            assert_eq!(
                output.encode_hex::<String>(),
                COMPRESSED_OFFER_HEX.encode_hex::<String>()
            );
        }
    }

    #[test]
    fn test_decompression() {
        for _ in MIN_VERSION..=MAX_VERSION {
            let output = decompress_offer_bytes(&COMPRESSED_OFFER_HEX).unwrap();

            assert_eq!(
                output.encode_hex::<String>(),
                DECOMPRESSED_OFFER_HEX.encode_hex::<String>()
            );
        }
    }

    #[test]
    fn parse_spend_bundle() {
        SpendBundle::from_bytes(&DECOMPRESSED_OFFER_HEX).unwrap();
    }

    const COMPRESSED_OFFER_HEX: [u8; 1225] = hex!(
        "
        000678bb1ce2864b63606060622000f234ef6ae4725635979d975e6263fb68c9b687879b720f54fbcf0ace3de319c33d09a60ede4e1dadfd476bffd1da7fb4f61fadfd476bffff0355fb830bf705e6fb3e27bc6b6d34de3637326a42ee9158c6f501e673d706c6ed0cecda5d23ab550555c6f445a3f9b7b1852187eac9c0f7675608bcd9265e74f3e9ad2c5d5faed4893b3aeba055c5688b8288ee3f234466c1f5c59bc3dfd96dbcdc5aa327e6fd6ee9e99e8f172eb31fde71c2cdb66561eca3c7af804aa6d4443f0cd2fa1f6eb6577ba710cb9fc5ad697cf64be7596f5ffcae607e50c3fcc2ffade21e652f18885009b273febfbd99c12d7de60d5c57358d8e1dbbc795a9b0a6d4e9ea910ef9d007a50deed925613fb414ea5ca30f2eaff9a0f4cff964c7a693676d2fdd7448ba1514b7f1505130b419038f6678188fae18185d31008ef9d1150330c121b3620092b617d4af320ade7ff7e2837ba739d936dd4972df7de47c427f8449acb8d5ede4175b6ae6ff5ff0728acd3aa766d7c7f97cbeba7667cf17fb3d3bd6da671653ffe68cd6bc97befbad17bcff50aae419907020ed5bbed19a43cbebacec8aab4e75af5b727199df92b3076d57a2d611ff47d729d02e018cae5318beeb14c0097ec102dff9e165fe214bec1c0cf6fdf66d60ca6b67955cb52cd964dd0f03ff58e6c98760f9a2f4ff81c88c929282622b7dfda4c4b4eca2d4cca48ca28ad2a2ca34a3f4f2caf4e28ad4ec9c8cc4f492dc948a92e2fce4c2b2acacaa94c24a3393a2ac9432e3a294bc94c2bc7413bdcc82b462bdbcb492e292fca2c4f454bd9cccbc6cd0b044c602f5de9732e26edb78b5277a5bf09af17d9edccab5d452c6f09dabf2ba47667dbce6ffff37e596fe5f087344416271496a52669e5e727eae7e5162b97e88a98f7b8153aa71034861c682c008a6f8681599bc832d9d45963116a61e53e2d30e2672ff3a706f25fbb6b8de04a0aae23cd0fc567109ac665ef0afcbdb4f7dd1a359bef6c7cfcd0daaba7deaccae6d6b174fb0396fbd7279e6a5ebdd584a9ed1651e28096c7499c7e8320f02eef93fbacc83d878a2cf320f48d97774f5abcaf5ab18a443d4ef2f3871dee954bec5d716e98a9f9b0f0515db6dbaa7ae0655f6b0ab32d0e47262d7295619afd882af4b7c76f1e9aee7c830e0e672d73b14f53d0f4b1139ba9c627439c550584ef19fce3d98058b9d16dabff3fbb2ebedec9fe9db2f4f375cbab5f77ef5c4d939f939dfee2e5e3af10c4419176ca0075bc61ab6db1bc1066fe87931a1f4d68ffcc0c247870433ef3c8b78eae9b54e6642cd87e78eb9c71faadd158fdb9f67bae7e5b740d35b9d53ea1e8a9e428c8ba150ff09c7d9ff0561ed81a593f6ef60faee68c15eebbe42397dcad515b981b24d816a1e7a96d3e7f2805d061abc013b18d87803316003670b08cddd413202b01bb4e0fcfe7b6f637c16ddb3e5feaab0f3b4f7f6122bd3d9979e2decb838e7f9fd3f8a979f830c27a8086496fdff0552f1cc53263ead78fe2ffafe9edf3b5e6e49a98cf874ffa813df0ae3b5f6bf2a5bf361f9b569fdd32dedbf5a8e9cbbe35da7cd7dfda2a038fff584f2522b734be70949afef545888cc3df25eee72421443edffd2dfd3bef2a9b15e5e3ed3e34ca2e57efe075e923e3badde58d8258b35873ebebb69fd156571cbe7f20fa6a4fbea7c0f5772da3e554d190018bbefd7
        "
    );

    const DECOMPRESSED_OFFER_HEX: [u8; 7390] = hex!(
        "
        0000000200000000000000000000000000000000000000000000000000000000000000006e29dd286d097a8376cf1ba43c3de2a4b6e1c3826dc07b4f9a536dcc495c0b920000000000000000ff02ffff01ff02ffff01ff02ff5effff04ff02ffff04ffff04ff05ffff04ffff0bff34ff0580ffff04ff0bff80808080ffff04ffff02ff17ff2f80ffff04ff5fffff04ffff02ff2effff04ff02ffff04ff17ff80808080ffff04ffff02ff2affff04ff02ffff04ff82027fffff04ff82057fffff04ff820b7fff808080808080ffff04ff81bfffff04ff82017fffff04ff8202ffffff04ff8205ffffff04ff820bffff80808080808080808080808080ffff04ffff01ffffffff3d46ff02ff333cffff0401ff01ff81cb02ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04ff0dffff04ffff0bff7cffff0bff34ff2480ffff0bff7cffff0bff7cffff0bff34ff2c80ff0980ffff0bff7cff0bffff0bff34ff8080808080ff8080808080ffff010b80ff0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ffff0dff0b80ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780ffff01ff088080ff0180ffff02ffff03ff0bffff01ff02ffff03ffff09ffff02ff2effff04ff02ffff04ff13ff80808080ff820b9f80ffff01ff02ff56ffff04ff02ffff04ffff02ff13ffff04ff5fffff04ff17ffff04ff2fffff04ff81bfffff04ff82017fffff04ff1bff8080808080808080ffff04ff82017fff8080808080ffff01ff088080ff0180ffff01ff02ffff03ff17ffff01ff02ffff03ffff20ff81bf80ffff0182017fffff01ff088080ff0180ffff01ff088080ff018080ff0180ff04ffff04ff05ff2780ffff04ffff10ff0bff5780ff778080ffffff02ffff03ff05ffff01ff02ffff03ffff09ffff02ffff03ffff09ff11ff5880ffff0159ff8080ff0180ffff01818f80ffff01ff02ff26ffff04ff02ffff04ff0dffff04ff0bffff04ffff04ff81b9ff82017980ff808080808080ffff01ff02ff7affff04ff02ffff04ffff02ffff03ffff09ff11ff5880ffff01ff04ff58ffff04ffff02ff76ffff04ff02ffff04ff13ffff04ff29ffff04ffff0bff34ff5b80ffff04ff2bff80808080808080ff398080ffff01ff02ffff03ffff09ff11ff7880ffff01ff02ffff03ffff20ffff02ffff03ffff09ffff0121ffff0dff298080ffff01ff02ffff03ffff09ffff0cff29ff80ff3480ff5c80ffff01ff0101ff8080ff0180ff8080ff018080ffff0109ffff01ff088080ff0180ffff010980ff018080ff0180ffff04ffff02ffff03ffff09ff11ff5880ffff0159ff8080ff0180ffff04ffff02ff26ffff04ff02ffff04ff0dffff04ff0bffff04ff17ff808080808080ff80808080808080ff0180ffff01ff04ff80ffff04ff80ff17808080ff0180ffff02ffff03ff05ffff01ff04ff09ffff02ff56ffff04ff02ffff04ff0dffff04ff0bff808080808080ffff010b80ff0180ff0bff7cffff0bff34ff2880ffff0bff7cffff0bff7cffff0bff34ff2c80ff0580ffff0bff7cffff02ff32ffff04ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0bff34ff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ffff04ffff04ff30ffff04ff5fff808080ffff02ff7effff04ff02ffff04ffff04ffff04ff2fff0580ffff04ff5fff82017f8080ffff04ffff02ff26ffff04ff02ffff04ff0bffff04ff05ffff01ff808080808080ffff04ff17ffff04ff81bfffff04ff82017fffff04ffff02ff2affff04ff02ffff04ff8204ffffff04ffff02ff76ffff04ff02ffff04ff09ffff04ff820affffff04ffff0bff34ff2d80ffff04ff15ff80808080808080ffff04ff8216ffff808080808080ffff04ff8205ffffff04ff820bffff808080808080808080808080ff02ff5affff04ff02ffff04ff5fffff04ff3bffff04ffff02ffff03ff17ffff01ff09ff2dffff02ff2affff04ff02ffff04ff27ffff04ffff02ff76ffff04ff02ffff04ff29ffff04ff57ffff04ffff0bff34ff81b980ffff04ff59ff80808080808080ffff04ff81b7ff80808080808080ff8080ff0180ffff04ff17ffff04ff05ffff04ff8202ffffff04ffff04ffff04ff78ffff04ffff0eff5cffff02ff2effff04ff02ffff04ffff04ff2fffff04ff82017fff808080ff8080808080ff808080ffff04ffff04ff20ffff04ffff0bff81bfff5cffff02ff2effff04ff02ffff04ffff04ff15ffff04ffff10ff82017fffff11ff8202dfff2b80ff8202ff80ff808080ff8080808080ff808080ff138080ff80808080808080808080ff018080ffff04ffff01a037bef360ee858133b69d595a906dc45d01af50379dad515eb9518abb7c1d2a7affff04ffff01a002f42883fb3338310825c951efcca810ecb61772d9e5da6a2d4d0a6591b8897effff04ffff01ff02ffff01ff02ff0affff04ff02ffff04ff03ff80808080ffff04ffff01ffff333effff02ffff03ff05ffff01ff04ffff04ff0cffff04ffff02ff1effff04ff02ffff04ff09ff80808080ff808080ffff02ff16ffff04ff02ffff04ff19ffff04ffff02ff0affff04ff02ffff04ff0dff80808080ff808080808080ff8080ff0180ffff02ffff03ff05ffff01ff02ffff03ffff15ff29ff8080ffff01ff04ffff04ff08ff0980ffff02ff16ffff04ff02ffff04ff0dffff04ff0bff808080808080ffff01ff088080ff0180ffff010b80ff0180ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff1effff04ff02ffff04ff09ff80808080ffff02ff1effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff018080ff0180808080ffffa0d7a3b357ee3eb1d3857c2e164beea5cb8cf1d0d307c3b8c8463d84a15de2e3eaffffa0947c5be1522aff5736bd2bb91204fca385660e3fa59e3bb7a3ee709f52809f71ff85174876e800ffffa0947c5be1522aff5736bd2bb91204fca385660e3fa59e3bb7a3ee709f52809f71808080809ffebd6953848e37800ad52932c6c6de0a6920ac7542d5c4881f55e07580476b7456f82a207e455bc1a77cf022fe43c988b2c9cd3dd2d94062da525eb1c272530000000000000001ff02ffff01ff02ffff01ff02ffff03ffff18ff2fff3480ffff01ff04ffff04ff20ffff04ff2fff808080ffff04ffff02ff3effff04ff02ffff04ff05ffff04ffff02ff2affff04ff02ffff04ff27ffff04ffff02ffff03ff77ffff01ff02ff36ffff04ff02ffff04ff09ffff04ff57ffff04ffff02ff2effff04ff02ffff04ff05ff80808080ff808080808080ffff011d80ff0180ffff04ffff02ffff03ff77ffff0181b7ffff015780ff0180ff808080808080ffff04ff77ff808080808080ffff02ff3affff04ff02ffff04ff05ffff04ffff02ff0bff5f80ffff01ff8080808080808080ffff01ff088080ff0180ffff04ffff01ffffffff4947ff0233ffff0401ff0102ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04ff0dffff04ffff0bff3cffff0bff34ff2480ffff0bff3cffff0bff3cffff0bff34ff2c80ff0980ffff0bff3cff0bffff0bff34ff8080808080ff8080808080ffff010b80ff0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ffff0dff0b80ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780ffff01ff088080ff0180ff02ffff03ff0bffff01ff02ffff03ffff02ff26ffff04ff02ffff04ff13ff80808080ffff01ff02ffff03ffff20ff1780ffff01ff02ffff03ffff09ff81b3ffff01818f80ffff01ff02ff3affff04ff02ffff04ff05ffff04ff1bffff04ff34ff808080808080ffff01ff04ffff04ff23ffff04ffff02ff36ffff04ff02ffff04ff09ffff04ff53ffff04ffff02ff2effff04ff02ffff04ff05ff80808080ff808080808080ff738080ffff02ff3affff04ff02ffff04ff05ffff04ff1bffff04ff34ff8080808080808080ff0180ffff01ff088080ff0180ffff01ff04ff13ffff02ff3affff04ff02ffff04ff05ffff04ff1bffff04ff17ff8080808080808080ff0180ffff01ff02ffff03ff17ff80ffff01ff088080ff018080ff0180ffffff02ffff03ffff09ff09ff3880ffff01ff02ffff03ffff18ff2dffff010180ffff01ff0101ff8080ff0180ff8080ff0180ff0bff3cffff0bff34ff2880ffff0bff3cffff0bff3cffff0bff34ff2c80ff0580ffff0bff3cffff02ff32ffff04ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0bff34ff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ffff21ff17ffff09ff0bff158080ffff01ff04ff30ffff04ff0bff808080ffff01ff088080ff0180ff018080ffff04ffff01ffa07faa3253bfddd1e0decb0906b2dc6247bbc4cf608f58345d173adb63e8b47c9fffa0e9943cae428345e36f0e4d2d3ecdcf734ee6c6858e365c7feccc2a9ee94dbf3ba0eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9ffff04ffff01ff02ffff01ff02ffff01ff02ff3effff04ff02ffff04ff05ffff04ffff02ff2fff5f80ffff04ff80ffff04ffff04ffff04ff0bffff04ff17ff808080ffff01ff808080ffff01ff8080808080808080ffff04ffff01ffffff0233ff04ff0101ffff02ff02ffff03ff05ffff01ff02ff1affff04ff02ffff04ff0dffff04ffff0bff12ffff0bff2cff1480ffff0bff12ffff0bff12ffff0bff2cff3c80ff0980ffff0bff12ff0bffff0bff2cff8080808080ff8080808080ffff010b80ff0180ffff0bff12ffff0bff2cff1080ffff0bff12ffff0bff12ffff0bff2cff3c80ff0580ffff0bff12ffff02ff1affff04ff02ffff04ff07ffff04ffff0bff2cff2c80ff8080808080ffff0bff2cff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ff0bffff01ff02ffff03ffff09ff23ff1880ffff01ff02ffff03ffff18ff81b3ff2c80ffff01ff02ffff03ffff20ff1780ffff01ff02ff3effff04ff02ffff04ff05ffff04ff1bffff04ff33ffff04ff2fffff04ff5fff8080808080808080ffff01ff088080ff0180ffff01ff04ff13ffff02ff3effff04ff02ffff04ff05ffff04ff1bffff04ff17ffff04ff2fffff04ff5fff80808080808080808080ff0180ffff01ff02ffff03ffff09ff23ffff0181e880ffff01ff02ff3effff04ff02ffff04ff05ffff04ff1bffff04ff17ffff04ffff02ffff03ffff22ffff09ffff02ff2effff04ff02ffff04ff53ff80808080ff82014f80ffff20ff5f8080ffff01ff02ff53ffff04ff818fffff04ff82014fffff04ff81b3ff8080808080ffff01ff088080ff0180ffff04ff2cff8080808080808080ffff01ff04ff13ffff02ff3effff04ff02ffff04ff05ffff04ff1bffff04ff17ffff04ff2fffff04ff5fff80808080808080808080ff018080ff0180ffff01ff04ffff04ff18ffff04ffff02ff16ffff04ff02ffff04ff05ffff04ff27ffff04ffff0bff2cff82014f80ffff04ffff02ff2effff04ff02ffff04ff818fff80808080ffff04ffff0bff2cff0580ff8080808080808080ff378080ff81af8080ff0180ff018080ffff04ffff01a0a04d9f57764f54a43e4030befb4d80026e870519aaa66334aef8304f5d0393c2ffff04ffff01ffff75ffc05968747470733a2f2f6261666b726569626872787572796632677779677378656b6c686167746d647874736f6371766a6a7a6471793634726a64763372646e64716e67342e697066732e6e667473746f726167652e6c696e6b2f80ffff68a0278de91c1746b60d2b914b380d360ef393850aa5391c31ee4523aee2368e0d37ffff826d75ffa168747470733a2f2f706173746562696e2e636f6d2f7261772f54354c477042653380ffff826d68a05158025f5b241c6ec1848972395c383548945f66c1610bfac0dea907b65e8d60ffff82736e01ffff8273740180ffff04ffff01a0fe8a4b4e27a2e29a4d3fc7ce9d527adbcaccbab6ada3903ccf3ba9a769d2d78bffff04ffff01ff02ffff01ff02ffff01ff02ff26ffff04ff02ffff04ff05ffff04ff17ffff04ff0bffff04ffff02ff2fff5f80ff80808080808080ffff04ffff01ffffff82ad4cff0233ffff3e04ff81f601ffffff0102ffff02ffff03ff05ffff01ff02ff2affff04ff02ffff04ff0dffff04ffff0bff32ffff0bff3cff3480ffff0bff32ffff0bff32ffff0bff3cff2280ff0980ffff0bff32ff0bffff0bff3cff8080808080ff8080808080ffff010b80ff0180ff04ffff04ff38ffff04ffff02ff36ffff04ff02ffff04ff05ffff04ff27ffff04ffff02ff2effff04ff02ffff04ffff02ffff03ff81afffff0181afffff010b80ff0180ff80808080ffff04ffff0bff3cff4f80ffff04ffff0bff3cff0580ff8080808080808080ff378080ff82016f80ffffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff2fffff01ff80ff808080808080808080ff0bff32ffff0bff3cff2880ffff0bff32ffff0bff32ffff0bff3cff2280ff0580ffff0bff32ffff02ff2affff04ff02ffff04ff07ffff04ffff0bff3cff3c80ff8080808080ffff0bff3cff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ff5fffff01ff02ffff03ffff09ff82011fff3880ffff01ff02ffff03ffff09ffff18ff82059f80ff3c80ffff01ff02ffff03ffff20ff81bf80ffff01ff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff82019fffff04ff82017fff80808080808080808080ffff01ff088080ff0180ffff01ff04ff819fffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ff82017fff808080808080808080808080ff0180ffff01ff02ffff03ffff09ff82011fff2c80ffff01ff02ffff03ffff20ff82017f80ffff01ff04ffff04ff24ffff04ffff0eff10ffff02ff2effff04ff02ffff04ff82019fff8080808080ff808080ffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ffff02ff0bffff04ff17ffff04ff2fffff04ff82019fff8080808080ff8080808080808080808080ffff01ff088080ff0180ffff01ff02ffff03ffff09ff82011fff2480ffff01ff02ffff03ffff20ffff02ffff03ffff09ffff0122ffff0dff82029f8080ffff01ff02ffff03ffff09ffff0cff82029fff80ffff010280ff1080ffff01ff0101ff8080ff0180ff8080ff018080ffff01ff04ff819fffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ff82017fff8080808080808080808080ffff01ff088080ff0180ffff01ff04ff819fffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ff82017fff808080808080808080808080ff018080ff018080ff0180ffff01ff02ff3affff04ff02ffff04ff05ffff04ff0bffff04ff81bfffff04ffff02ffff03ff82017fffff0182017fffff01ff02ff0bffff04ff17ffff04ff2fffff01ff808080808080ff0180ff8080808080808080ff0180ff018080ffff04ffff01a0c5abea79afaa001b5427dfa0c8cf42ca6f38f5841b78f9b3c252733eb2de2726ffff04ffff01a0e18a795134d3618aca051c4a5d70f5a44cba0e2daf0868300b0a472ec25af76effff04ffff01ff02ffff01ff02ffff01ff02ffff03ff81bfffff01ff04ff82013fffff04ff80ffff04ffff02ffff03ffff22ff82013fffff20ffff09ff82013fff2f808080ffff01ff04ffff04ff10ffff04ffff0bffff02ff2effff04ff02ffff04ff09ffff04ff8205bfffff04ffff02ff3effff04ff02ffff04ffff04ff09ffff04ff82013fff1d8080ff80808080ff808080808080ff1580ff808080ffff02ff16ffff04ff02ffff04ff0bffff04ff17ffff04ff8202bfffff04ff15ff8080808080808080ffff01ff02ff16ffff04ff02ffff04ff0bffff04ff17ffff04ff8202bfffff04ff15ff8080808080808080ff0180ff80808080ffff01ff04ff2fffff01ff80ff80808080ff0180ffff04ffff01ffffff3f02ff04ff0101ffff822710ff02ff02ffff03ff05ffff01ff02ff3affff04ff02ffff04ff0dffff04ffff0bff2affff0bff2cff1480ffff0bff2affff0bff2affff0bff2cff3c80ff0980ffff0bff2aff0bffff0bff2cff8080808080ff8080808080ffff010b80ff0180ffff02ffff03ff17ffff01ff04ffff04ff10ffff04ffff0bff81a7ffff02ff3effff04ff02ffff04ffff04ff2fffff04ffff04ff05ffff04ffff05ffff14ffff12ff47ff0b80ff128080ffff04ffff04ff05ff8080ff80808080ff808080ff8080808080ff808080ffff02ff16ffff04ff02ffff04ff05ffff04ff0bffff04ff37ffff04ff2fff8080808080808080ff8080ff0180ffff0bff2affff0bff2cff1880ffff0bff2affff0bff2affff0bff2cff3c80ff0580ffff0bff2affff02ff3affff04ff02ffff04ff07ffff04ffff0bff2cff2c80ff8080808080ffff0bff2cff8080808080ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff3effff04ff02ffff04ff09ff80808080ffff02ff3effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff018080ffff04ffff01ffa07faa3253bfddd1e0decb0906b2dc6247bbc4cf608f58345d173adb63e8b47c9fffa0e9943cae428345e36f0e4d2d3ecdcf734ee6c6858e365c7feccc2a9ee94dbf3ba0eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9ffff04ffff01a0a342a13fee4ef4baed9bf967b7d39731a5b58ddf7b919b6c6f6cf6dda3a591ccffff04ffff010aff0180808080ffff04ffff01ff02ffff01ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff1dff0bffff1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080808080ffff01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff04ffff04ff05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff80808080ffff02ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ffff02ff06ffff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff018080ffff04ffff01b08ce89075daf86f5171e2c21169dce658e5494aae1c907cf0e7416dc7e126dd175ebf6e35bce9f65135da89947ee115caff018080ff018080808080ff018080808080ff01808080ffffa0e9943cae428345e36f0e4d2d3ecdcf734ee6c6858e365c7feccc2a9ee94dbf3bffa05687517592bfb802f74138077d47a8236794d5a86d511d825126482e39979d0cff0180ff01ffffffff80ffff01ffff81f6ff80ffffff85174876e800ffa06e29dd286d097a8376cf1ba43c3de2a4b6e1c3826dc07b4f9a536dcc495c0b928080ff8080ffff33ffa0cfbfdeed5c4ca2de3d0bf520b9cb4bb7743a359bd2e6a188d19ce7dffc21d3e7ff01ffffa0cfbfdeed5c4ca2de3d0bf520b9cb4bb7743a359bd2e6a188d19ce7dffc21d3e78080ffff3fffa01a5f039491e578e7fe5bdfbcfbb8e9b4647958f2dfc5420ea833ad3ffa79856f8080ff808080808082afe5b487fa84c4cedc4b7e2b0bd7d111170fd76077753a3739439062ebdc7838149dc4ef1ed3605a007dff75fb96f50e2605d3a79948cc6139bf0fe04a194cb93aec383e63168355e3ddb2afd4231739e71fe094674d2cf7572242b7952623
        "
    );
}
