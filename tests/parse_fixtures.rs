use faa_registry_mirror::parse::{parse_acftref, parse_dereg, parse_docindex, parse_engine, parse_master};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).expect(name)
}

#[test]
fn fixture_master_offsets() {
    let parsed = parse_master(&fixture("MASTER.txt"));
    assert!(parsed.errors.is_empty());
    assert_eq!(parsed.records[0].n_number, "N12345");
    assert_eq!(parsed.records[0].owner_name, "BANK OF UTAH TRUSTEE");
    assert_eq!(parsed.records[0].state, "CO");
    assert_eq!(parsed.records[0].mode_s_hex, "A00B1C");
}

#[test]
fn fixture_refs_and_dereg_and_docs() {
    let ac = parse_acftref(&fixture("ACFTREF.txt"));
    assert_eq!(ac.records[0].mfr, "CESSNA");
    assert_eq!(ac.records[0].model, "172S");

    let eng = parse_engine(&fixture("ENGINE.txt"));
    assert_eq!(eng.records[0].model, "IO-360-L2A,A");

    let dereg = parse_dereg(&fixture("DEREG.txt"));
    assert_eq!(dereg.records[0].export_country, "CANADA");

    let docs = parse_docindex(&fixture("DOCINDEX.txt"));
    assert_eq!(docs.records[0].n_number, "N12345");
    assert_eq!(docs.records[0].party_name, "BANK, N.A.");
    assert_eq!(docs.records[0].doc_type, "SECURITY");
}

#[test]
fn live_dump_samples_parse() {
    let master = parse_master(&fixture("live_MASTER.txt"));
    assert!(master.errors.is_empty(), "{:?}", master.errors);
    assert_eq!(master.records.len(), 2);
    assert_eq!(master.records[0].n_number, "N100");
    assert_eq!(master.records[0].owner_name, "BENE MARY D");
    assert_eq!(master.records[0].city, "KETCHUM");
    assert_eq!(master.records[0].state, "OK");
    assert_eq!(master.records[0].status_code, "V");
    assert_eq!(master.records[0].mode_s_hex, "A004B3");
    assert_eq!(master.records[1].n_number, "N10000");
    assert_eq!(master.records[1].owner_name, "9AT LLC");

    let ac = parse_acftref(&fixture("live_ACFTREF.txt"));
    assert_eq!(ac.records[0].code, "0020901");
    assert_eq!(ac.records[0].mfr, "AAR AIRLIFT GROUP INC");
    assert_eq!(ac.records[0].model, "UH-60A");

    let eng = parse_engine(&fixture("live_ENGINE.txt"));
    assert_eq!(eng.records[0].code, "00000");
    assert_eq!(eng.records[0].mfr, "NONE");

    let dereg = parse_dereg(&fixture("live_DEREG.txt"));
    assert!(dereg.errors.is_empty(), "{:?}", dereg.errors);
    assert_eq!(dereg.records[0].n_number, "N1");
    assert_eq!(dereg.records[0].owner_name, "KEMNITZER GEORGE E");
    assert_eq!(dereg.records[0].cancel_date, "1939-09-01");
    assert_eq!(dereg.records[0].city, "NEWARK");
    assert_eq!(dereg.records[0].state, "OH");
    assert_eq!(dereg.records[0].mode_s_hex, "A00001");

    let docs = parse_docindex(&fixture("live_DOCINDEX.txt"));
    assert!(docs.errors.is_empty(), "{:?}", docs.errors);
    assert_eq!(docs.records[0].n_number, "N1006W");
    assert_eq!(docs.records[0].document_id, "ARE017980731");
    assert_eq!(docs.records[0].doc_type, "BOS");
}
