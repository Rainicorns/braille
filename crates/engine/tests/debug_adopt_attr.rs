use braille_engine::Engine;

#[test]
fn debug_adopt_attr_owner_document() {
    let mut engine = Engine::new();
    engine.load_html(r#"<!DOCTYPE html><html><head></head><body></body></html>"#);

    let result = engine.eval_js(r#"
        var div = document.createElement("div");
        div.id = "foobar";

        var attrDoc1 = div.attributes[0].ownerDocument;
        var divDoc1 = div.ownerDocument;

        var other_doc = document.implementation.createHTMLDocument();

        // Debug: check other_doc.body exists
        var bodyExists = !!other_doc.body;
        var bodyOwnerDoc = other_doc.body ? other_doc.body.ownerDocument : 'no body';
        var bodyOwnerIsOtherDoc = bodyOwnerDoc === other_doc;

        other_doc.body.appendChild(div);

        var divDoc2 = div.ownerDocument;
        var divOwnerDocIsOther = divDoc2 === other_doc;
        var divOwnerDocDirect = div.__ownerDoc;
        var divOwnerDocDirectNodeType = divOwnerDocDirect ? divOwnerDocDirect.nodeType : 'none';
        var divOwnerDocDirectIsDoc = divOwnerDocDirect === document;
        var otherDocNid = other_doc.__nid;

        var attr = div.attributes[0];
        var attrOwnerDoc = attr.__ownerDoc;
        var attrOwnerDocResult = attr.ownerDocument;
        var attrIsOther = attrOwnerDocResult === other_doc;
        var elOwnerDoc = div.__ownerDoc;
        var elOwnerIsOther = elOwnerDoc === other_doc;

        JSON.stringify({
            bodyExists: bodyExists,
            bodyOwnerIsOtherDoc: bodyOwnerIsOtherDoc,
            divOwnerDocIsOther: divOwnerDocIsOther,
            elOwnerIsOther: elOwnerIsOther,
            attrIsOther: attrIsOther,
            attrHasOwnerDoc: !!attrOwnerDoc,
            attrOwnerDocType: typeof attrOwnerDoc,
            divOwnerDocDirectNodeType: divOwnerDocDirectNodeType,
            divOwnerDocDirectIsDoc: divOwnerDocDirectIsDoc,
            otherDocNid: otherDocNid,
            divDoc2NodeType: divDoc2 ? divDoc2.nodeType : 'null',
            attrOwnerDocNodeType: attrOwnerDocResult ? attrOwnerDocResult.nodeType : 'null'
        });
    "#);

    eprintln!("Result: {:?}", result);
}
