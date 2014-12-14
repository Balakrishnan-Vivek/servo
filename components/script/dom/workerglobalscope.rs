/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::WorkerGlobalScopeBinding::WorkerGlobalScopeMethods;
use dom::bindings::codegen::Bindings::FunctionBinding::Function;
use dom::bindings::error::{ErrorResult, Fallible, Syntax, Network, FailureUnknown};
use dom::bindings::global;
use dom::bindings::js::{MutNullableJS, JSRef, Temporary};
use dom::bindings::utils::{Reflectable, Reflector};
use dom::console::Console;
use dom::eventtarget::{EventTarget, EventTargetTypeId};
use dom::workerlocation::WorkerLocation;
use dom::workernavigator::WorkerNavigator;
use dom::window::{base64_atob, base64_btoa};
use script_task::{ScriptChan, FromWorker};
use timers::{Interval, NonInterval, TimerId, TimerManager};

use servo_net::resource_task::{ResourceTask, load_whole_resource};
use servo_util::str::DOMString;

use js::jsapi::JSContext;
use js::jsval::JSVal;
use js::rust::Cx;

use std::default::Default;
use std::rc::Rc;
use url::{Url, UrlParser};

#[deriving(PartialEq)]
#[jstraceable]
pub enum WorkerGlobalScopeTypeId {
    DedicatedGlobalScope,
}

#[dom_struct]
pub struct WorkerGlobalScope {
    eventtarget: EventTarget,
    worker_url: Url,
    js_context: Rc<Cx>,
    resource_task: ResourceTask,
    script_chan: ScriptChan,
    location: MutNullableJS<WorkerLocation>,
    navigator: MutNullableJS<WorkerNavigator>,
    console: MutNullableJS<Console>,
    timers: TimerManager,
}

impl WorkerGlobalScope {
    pub fn new_inherited(type_id: WorkerGlobalScopeId,
                         worker_url: Url,
                         cx: Rc<Cx>,
                         resource_task: ResourceTask,
                         script_chan: ScriptChan) -> WorkerGlobalScope {
        WorkerGlobalScope {
            eventtarget: EventTarget::new_inherited(EventTargetTypeId::WorkerGlobalScope(type_id)),
            worker_url: worker_url,
            js_context: cx,
            resource_task: resource_task,
            script_chan: script_chan,
            location: Default::default(),
            navigator: Default::default(),
            console: Default::default(),
            timers: TimerManager::new()
        }
    }

    #[inline]
    pub fn eventtarget<'a>(&'a self) -> &'a EventTarget {
        &self.eventtarget
    }

    pub fn get_cx(&self) -> *mut JSContext {
        self.js_context.ptr
    }

    pub fn resource_task<'a>(&'a self) -> &'a ResourceTask {
        &   self.resource_task
    }

    pub fn get_url<'a>(&'a self) -> &'a Url {
        &self.worker_url
    }

    pub fn script_chan<'a>(&'a self) -> &'a ScriptChan {
        &self.script_chan
    }
}

impl<'a> WorkerGlobalScopeMethods for JSRef<'a, WorkerGlobalScope> {
    fn Self(self) -> Temporary<WorkerGlobalScope> {
        Temporary::from_rooted(self)
    }

    fn Location(self) -> Temporary<WorkerLocation> {
        self.location.or_init(|| {
            WorkerLocation::new(self, self.worker_url.clone())
        })
    }

    fn ImportScripts(self, url_strings: Vec<DOMString>) -> ErrorResult {
        let mut urls = Vec::with_capacity(url_strings.len());
        for url in url_strings.into_iter() {
            let url = UrlParser::new().base_url(&self.worker_url)
                                      .parse(url.as_slice());
            match url {
                Ok(url) => urls.push(url),
                Err(_) => return Err(Syntax),
            };
        }

        for url in urls.into_iter() {
            let (url, source) = match load_whole_resource(&self.resource_task, url) {
                Err(_) => return Err(Network),
                Ok((metadata, bytes)) => {
                    (metadata.final_url, String::from_utf8(bytes).unwrap())
                }
            };

            match self.js_context.evaluate_script(
                self.reflector().get_jsobject(), source, url.serialize(), 1) {
                Ok(_) => (),
                Err(_) => {
                    println!("evaluate_script failed");
                    return Err(FailureUnknown);
                }
            }
        }

        Ok(())
    }

    fn Navigator(self) -> Temporary<WorkerNavigator> {
        self.navigator.or_init(|| WorkerNavigator::new(self))
    }

    fn Console(self) -> Temporary<Console> {
        self.console.or_init(|| Console::new(global::Worker(self)))
    }

    fn Btoa(self, btoa: DOMString) -> Fallible<DOMString> {
        base64_btoa(btoa)
    }

    fn Atob(self, atob: DOMString) -> Fallible<DOMString> {
        base64_atob(atob)
    }

    fn SetTimeout(self, _cx: *mut JSContext, callback: Function, timeout: i32, args: Vec<JSVal>) -> i32 {
        self.timers.set_timeout_or_interval(callback,
                                            args,
                                            timeout,
                                            NonInterval,
                                            FromWorker,
                                            self.script_chan.clone())
    }

    fn ClearTimeout(self, handle: i32) {
        self.timers.clear_timeout_or_interval(handle);
    }

    fn SetInterval(self, _cx: *mut JSContext, callback: Function, timeout: i32, args: Vec<JSVal>) -> i32 {
        self.timers.set_timeout_or_interval(callback,
                                            args,
                                            timeout,
                                            Interval,
                                            FromWorker,
                                            self.script_chan.clone())
    }

    fn ClearInterval(self, handle: i32) {
        self.ClearTimeout(handle);
    }
}

pub trait WorkerGlobalScopeHelpers {
    fn handle_fire_timer(self, timer_id: TimerId);
}

impl<'a> WorkerGlobalScopeHelpers for JSRef<'a, WorkerGlobalScope> {

    fn handle_fire_timer(self, timer_id: TimerId) {
        self.timers.fire_timer(timer_id, self.clone());
    }

}

impl Reflectable for WorkerGlobalScope {
    fn reflector<'a>(&'a self) -> &'a Reflector {
        self.eventtarget.reflector()
    }
}
