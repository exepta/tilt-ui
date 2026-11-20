use super::*;
use serde_json::json;
use std::cell::Cell;

#[test]
fn compound_inventory_expressions_match_owned_evaluation_without_cloning_containers() {
    let roots = json!({
        "inventoryView": {
            "open": true,
            "assembly": false,
            "container": false,
            "title": "Inventory",
            "items": (0..512).map(|index| json!({"id": index, "count": 128})).collect::<Vec<_>>()
        },
        "slot": {"empty": false, "count": "128", "command": "catalog", "selected": true},
        "index": 17,
        "key": "title",
        "word": "Gr\u{00fc}\u{00df}e"
    });
    for source in [
        "!inventoryView.open",
        "inventoryView.open && !inventoryView.assembly",
        "inventoryView.container ? 'storage' : (inventoryView.open ? 'inventory' : '')",
        "slot.command == 'craft' && slot.empty",
        "slot.selected ? 'item-cell selected' : 'item-cell'",
        "inventoryView.items[index].id == 17 && inventoryView.items[511].count == 128",
        "inventoryView[key].startsWith('Invent')",
        "inventoryView.items.length > 500 ? slot.count : ''",
        "word.length == 5 && word.contains('\u{00df}')",
        "inventoryView.items[-1] == null",
        "inventoryView.items[512] == null",
        "inventoryView.missing.foo == null",
        "false && inventoryView.fail()",
        "true || inventoryView.fail()",
        "true ? inventoryView.title : inventoryView.fail()",
        "(inventoryView).title",
        "slot.count + ' items'",
        "slot.count == '128' ? 2 * 4 + 1 : 0",
        "slot.count +",
        "inventoryView.items[",
    ] {
        let owned = evaluate_with_methods(source, |root| roots.get(root).cloned(), |_, _, _| None);
        let borrowed = evaluate_with_borrowed_methods(
            source,
            |root| roots.get(root).map(Cow::Borrowed),
            |_, _, _| None,
        );
        assert_eq!(borrowed, owned, "{source}");
    }
}

#[test]
fn compound_member_and_index_traversal_keeps_original_subtree_references() {
    let root = json!({"items": [{"id": 3}], "title": "Inventory"});
    let field = field_value(Cow::Borrowed(&root), "items");
    assert!(matches!(&field, Cow::Borrowed(value) if std::ptr::eq(*value, &root["items"])));
    let item = indexed_value(field, json!(0));
    assert!(matches!(&item, Cow::Borrowed(value) if std::ptr::eq(*value, &root["items"][0])));
    let id = field_value(item, "id");
    assert!(matches!(&id, Cow::Borrowed(value) if std::ptr::eq(*value, &root["items"][0]["id"])));
    assert_eq!(id.into_owned(), json!(3));
    assert_eq!(root["items"][0]["id"], json!(3));
}

#[test]
fn borrowed_methods_receive_original_receiver_and_keep_short_circuit_call_counts() {
    let root = json!({"title": "Inventory", "items": [1, 2, 3]});
    let calls = Cell::new(0);
    let lookup = |name: &str| (name == "model").then_some(Cow::Borrowed(&root));
    let call = |path: &str, receiver: &Value, args: &[Value]| {
        calls.set(calls.get() + 1);
        assert_eq!(path, "model.title_for");
        assert!(std::ptr::eq(receiver, &root));
        assert_eq!(args, &[json!(3)]);
        Some(receiver["title"].clone())
    };
    assert_eq!(
        evaluate_with_borrowed_methods("model.title_for(model.items.length)", lookup, call),
        Some(json!("Inventory")),
    );
    assert_eq!(calls.get(), 1);
    for source in [
        "true || model.title_for(3)",
        "false && model.title_for(3)",
        "false ? model.title_for(3) : 'skip'",
    ] {
        assert!(evaluate_with_borrowed_methods(source, lookup, call).is_some());
    }
    assert_eq!(calls.get(), 1);
}
