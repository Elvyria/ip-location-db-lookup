const IPV4_COUNTRY_NUM: &str = r#"16777216,16777471,AU
16777472,16778239,CN
16778240,16779263,AU
16779264,16781311,CN
16781312,16785407,JP
971448064,971448319,DE
971448320,971448575,GB
971448576,971448831,PL
971448832,971449087,DE
971449088,971449343,GB
971449600,971449855,GB
971449856,971450111,PL
971450112,971451391,GB
971451392,971451647,DE
971451648,971451903,PL
971451904,971452415,GB
971452416,971452671,PL
3653435392,3653439487,DE
3653439488,3653443583,FR
3653443584,3653447679,DE
3653447680,3653451775,LV
3653451776,3653464063,RU
3758092288,3758093311,HK
3758093312,3758094335,IN
3758094336,3758095359,HK
3758095360,3758095871,CN
3758095872,3758096127,SG
3758096128,3758096383,AU
"#;

#[test]
fn country_ipv4() {
    use std::net::Ipv4Addr;
    use crate::lookup_ipv4;

    let b = IPV4_COUNTRY_NUM.as_bytes();

    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(1, 0, 0, 0)), Some("AU"));
    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(1, 0, 0, 255)), Some("AU"));

    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(57, 231, 41, 241)),  Some("GB"));
    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(217, 195, 14, 20)),  Some("DE"));
    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(217, 195, 30, 255)), Some("FR"));

    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(223, 255, 255, 0)), Some("AU"));
    assert_eq!(lookup_ipv4(b, &Ipv4Addr::new(223, 255, 255, 255)), Some("AU"));
}

#[test]
fn find_nl() {
    use crate::find_nl;

    assert_eq!(find_nl(b"16777472,16778239,CN\n"), 20);
    assert_eq!(find_nl(b"223.255.252.0,223.255.253.255,CN\n"), 32);
    assert_eq!(find_nl(b"2.17.192.0,2.17.192.255,US\n"), 26);
    assert_eq!(find_nl(b"17301760,17302015,38345,Internet Domain Name System Beijing Engineering Resrarch Center Ltd.\n"), 92);
    assert_eq!(find_nl(b"17367040,17432575,4788,TM TECHNOLOGY SERVICES SDN BHD\n17435136,17435391,148000,National Knowledge Network\n"), 53);
}

#[test]
fn ipv4_to_num() {
    use std::net::Ipv4Addr;
    use crate::ipv4_num;

    assert_eq!(ipv4_num(&Ipv4Addr::new(1, 0, 0, 0)), 16777216);
    assert_eq!(ipv4_num(&Ipv4Addr::new(2, 16, 90, 0)), 34626048);
    assert_eq!(ipv4_num(&Ipv4Addr::new(217, 163, 135, 112)), 3651372912);
    assert_eq!(ipv4_num(&Ipv4Addr::new(223, 255, 255, 255)), 3758096383);
}

#[test]
fn str_to_num() {
    use crate::into_num;

    assert_eq!(into_num(b"16777216"),   16777216);
    assert_eq!(into_num(b"971448832"),  971448832);
    assert_eq!(into_num(b"3758096128"), 3758096128);
}
