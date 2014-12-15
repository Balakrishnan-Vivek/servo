/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::HTMLOListElementBinding;
use dom::bindings::codegen::InheritTypes::HTMLOListElementDerived;
use dom::bindings::js::{JSRef, Temporary};
use dom::bindings::utils::{Reflectable, Reflector};
use dom::document::Document;
use dom::element::ElementTypeId;
use dom::eventtarget::{EventTarget, EventTargetTypeId};
use dom::htmlelement::HTMLElement;
use dom::node::{Node, NodeTypeId};
use servo_util::str::DOMString;

#[dom_struct]
pub struct HTMLOListElement {
    htmlelement: HTMLElement,
}

impl HTMLOListElementDerived for EventTarget {
    fn is_htmlolistelement(&self) -> bool {
        *self.type_id() == EventTargetTypeId::Node(NodeTypeId::Element(ElementTypeId::HTMLOListElement))
    }
}

impl HTMLOListElement {
    fn new_inherited(localName: DOMString, prefix: Option<DOMString>, document: JSRef<Document>) -> HTMLOListElement {
        HTMLOListElement {
            htmlelement: HTMLElement::new_inherited(ElementTypeId::HTMLOListElement, localName, prefix, document)
        }
    }

    #[allow(unrooted_must_root)]
    pub fn new(localName: DOMString, prefix: Option<DOMString>, document: JSRef<Document>) -> Temporary<HTMLOListElement> {
        let element = HTMLOListElement::new_inherited(localName, prefix, document);
        Node::reflect_node(box element, document, HTMLOListElementBinding::Wrap)
    }
}

impl Reflectable for HTMLOListElement {
    fn reflector<'a>(&'a self) -> &'a Reflector {
        self.htmlelement.reflector()
    }
}
