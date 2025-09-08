// Copyright (c) 2019-2020 Emmanuel Gil Peyrot <linkmauve@linkmauve.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use xso::{AsXml, FromXml};

use crate::jingle_rtcp_fb::RtcpFb;
use crate::jingle_rtp_hdrext::RtpHdrext;
use crate::jingle_ssma::{Group, Source};
use crate::ns;

/// Specifies the ability to multiplex RTP Data and Control Packets on a single port as
/// described in RFC 5761.
#[derive(FromXml, AsXml, PartialEq, Debug, Clone)]
#[xml(namespace = ns::JINGLE_RTP, name = "rtcp-mux")]
pub struct RtcpMux;

/// Wrapper element describing an RTP session.
#[derive(FromXml, AsXml, PartialEq, Debug, Clone)]
#[xml(namespace = ns::JINGLE_RTP, name = "description")]
pub struct Description {
    /// Specifies the media type, such as "audio" or "video", where the media type SHOULD be as
    /// registered at IANA MIME Media Types Registry.
    #[xml(attribute)]
    pub media: String,

    /// 32-bit synchronization source for this media stream, as defined in RFC 3550.
    #[xml(attribute(default))]
    pub ssrc: Option<u32>,

    /// List of encodings that can be used for this RTP stream.
    #[xml(child(n = ..))]
    pub payload_types: Vec<PayloadType>,

    /// Specifies the ability to multiplex RTP Data and Control Packets on a single port as
    /// described in RFC 5761.
    #[xml(child(default))]
    pub rtcp_mux: Option<RtcpMux>,

    /// List of ssrc-group.
    #[xml(child(n = ..))]
    pub ssrc_groups: Vec<Group>,

    /// List of ssrc.
    #[xml(child(n = ..))]
    pub ssrcs: Vec<Source>,

    /// List of header extensions.
    #[xml(child(n = ..))]
    pub hdrexts: Vec<RtpHdrext>,
    // TODO: Add support for <encryption/> and <bandwidth/>.
}

impl Description {
    /// Create a new RTP description.
    pub fn new(media: String) -> Description {
        Description {
            media,
            ssrc: None,
            payload_types: Vec::new(),
            rtcp_mux: None,
            ssrc_groups: Vec::new(),
            ssrcs: Vec::new(),
            hdrexts: Vec::new(),
        }
    }
}

generate_attribute!(
    /// The number of channels.
    Channels,
    "channels",
    u8,
    Default = 1
);

/// An encoding that can be used for an RTP stream.
#[derive(FromXml, AsXml, PartialEq, Debug, Clone)]
#[xml(namespace = ns::JINGLE_RTP, name = "payload-type")]
pub struct PayloadType {
    /// The number of channels.
    #[xml(attribute(default))]
    pub channels: Channels,

    /// The sampling frequency in Hertz.
    #[xml(attribute(default))]
    pub clockrate: Option<u32>,

    /// The payload identifier.
    #[xml(attribute)]
    pub id: u8,

    /// Maximum packet time as specified in RFC 4566.
    #[xml(attribute(default))]
    pub maxptime: Option<u32>,

    /// The appropriate subtype of the MIME type.
    #[xml(attribute(default))]
    pub name: Option<String>,

    /// Packet time as specified in RFC 4566.
    #[xml(attribute(default))]
    pub ptime: Option<u32>,

    /// List of parameters specifying this payload-type.
    ///
    /// Their order MUST be ignored.
    #[xml(child(n = ..))]
    pub parameters: Vec<Parameter>,

    /// List of rtcp-fb parameters from XEP-0293.
    #[xml(child(n = ..))]
    pub rtcp_fbs: Vec<RtcpFb>,
}

impl PayloadType {
    /// Create a new RTP payload-type.
    pub fn new(id: u8, name: String, clockrate: u32, channels: u8) -> PayloadType {
        PayloadType {
            channels: Channels(channels),
            clockrate: Some(clockrate),
            id,
            maxptime: None,
            name: Some(name),
            ptime: None,
            parameters: Vec::new(),
            rtcp_fbs: Vec::new(),
        }
    }

    /// Create a new RTP payload-type without a clockrate.  Warning: this is invalid as per
    /// RFC 4566!
    pub fn without_clockrate(id: u8, name: String) -> PayloadType {
        PayloadType {
            channels: Default::default(),
            clockrate: None,
            id,
            maxptime: None,
            name: Some(name),
            ptime: None,
            parameters: Vec::new(),
            rtcp_fbs: Vec::new(),
        }
    }
}

/// Parameter related to a payload.
#[derive(FromXml, AsXml, PartialEq, Debug, Clone)]
#[xml(namespace = ns::JINGLE_RTP, name = "parameter")]
pub struct Parameter {
    /// The name of the parameter, from the list at
    /// <https://www.iana.org/assignments/sdp-parameters/sdp-parameters.xhtml>
    #[xml(attribute)]
    pub name: String,

    /// The value of this parameter.
    #[xml(attribute)]
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use minidom::Element;

    #[cfg(target_pointer_width = "32")]
    #[test]
    fn test_size() {
        assert_size!(Description, 72);
        assert_size!(Channels, 1);
        assert_size!(PayloadType, 64);
        assert_size!(Parameter, 24);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_size() {
        assert_size!(Description, 136);
        assert_size!(Channels, 1);
        assert_size!(PayloadType, 104);
        assert_size!(Parameter, 48);
    }

    #[test]
    fn test_simple() {
        let elem: Element = "<description xmlns='urn:xmpp:jingle:apps:rtp:1' media='audio'>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='2' clockrate='48000' id='96' name='OPUS'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='1' clockrate='32000' id='105' name='SPEEX'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='1' clockrate='8000' id='9' name='G722'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='1' clockrate='16000' id='106' name='SPEEX'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' clockrate='8000' id='8' name='PCMA'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' clockrate='8000' id='0' name='PCMU'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='1' clockrate='8000' id='107' name='SPEEX'/>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' channels='1' clockrate='8000' id='99' name='AMR'>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='octet-align' value='1'/>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='crc' value='0'/>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='robust-sorting' value='0'/>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='interleaving' value='0'/>
    </payload-type>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' clockrate='48000' id='100' name='telephone-event'>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='events' value='0-15'/>
    </payload-type>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' clockrate='16000' id='101' name='telephone-event'>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='events' value='0-15'/>
    </payload-type>
    <payload-type xmlns='urn:xmpp:jingle:apps:rtp:1' clockrate='8000' id='102' name='telephone-event'>
        <parameter xmlns='urn:xmpp:jingle:apps:rtp:1' name='events' value='0-15'/>
    </payload-type>
</description>"
                .parse()
                .unwrap();
        let desc = Description::try_from(elem).unwrap();
        assert_eq!(desc.media, "audio");
        assert_eq!(desc.ssrc, None);
    }
}
