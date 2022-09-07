/*!
 * An Allocator defines the interface for an allocation strategy for a router crossbar
*/

pub mod random;
pub mod random_priority;
pub mod islip;
//pub mod separable_input_first;

use crate::Plugs;
use crate::config_parser::ConfigurationValue;
use crate::router::basic_modular::PortRequest;

use std::cell::RefCell;
use ::rand::rngs::StdRng;
use random::RandomAllocator;
use random_priority::RandomPriorityAllocator;
use islip::IslipAllocator;


/// A client (input of crossbar) want a resource (output of crossbar) with a certain priority.
#[derive(Clone)]
pub struct Request {
    /// The input of the crossbar
    pub client: usize,
    /// The output of the crossbar
    pub resource: usize,
    /// The priority of the request (None if not specified)
    /// The priority is used to determine the order of the requests
    /// The lower the priority, the earlier the request is granted
    /// If the priority is 0, the request is an intransit request
    pub priority: Option<usize>,
}

impl Request {
    pub fn new(client: usize, resource: usize, priority: Option<usize>) -> Request { Self { client, resource, priority } }

    // method to transform a Request into a router::basic_ioq::PortRequest
    pub fn to_port_request(&self, num_vcs: usize)->PortRequest
	{
		PortRequest{
			entry_port: self.client/num_vcs,
			entry_vc: self.client%num_vcs,
			requested_port: self.resource/num_vcs,
			requested_vc: self.resource%num_vcs,
			label: if self.priority.is_none() {-1} else {self.priority.unwrap() as i32},
		}
	}
}

/// A collection of granted requests
#[derive(Default)]
pub struct GrantedRequests {
    /// The granted requests
    granted_requests: Vec<Request>,
}
impl GrantedRequests {
    /// Add a granted request to the collection
    fn add_granted_request(&mut self, request: Request) {
        self.granted_requests.push(request);
    }
}
impl Iterator for GrantedRequests {
    type Item = Request;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.granted_requests.is_empty() {
            let r = self.granted_requests.remove(0);
            Some(r)
        } else {
            None
        }
    }
}

pub trait Allocator {
    /// Add a new request to the allocator.
    /// (It assumes that the request is not already in the allocator)
    /// # Arguments
    /// * `request` - The request to add
    fn add_request(&mut self, request: Request);

    /// Returns the granted requests and clear the client's requests
    /// # Parameters
    /// * `rng` - The random number generator to use
    /// # Returns
    /// * `GrantedRequests` - The granted requests
    fn perform_allocation(&mut self, rng : &RefCell<StdRng>) -> GrantedRequests;

    /// Check if the allocator supports the intransit priority option
    /// # Returns
    /// * `bool` - True if the allocator supports the intransit priority option
    /// # Remarks
    /// The intransit priority option is used to specify the give more priority to the requests
    /// that come from the another router rather than a server.
    fn support_intransit_priority(&self) -> bool;
}

/// Arguments for the allocator builder
#[non_exhaustive]
pub struct AllocatorBuilderArgument<'a>
{
    /// A ConfigurationValue::Object defining the allocator
    pub cv : &'a ConfigurationValue,
    /// The number of outputs of the router crossbar
    pub num_resources : usize,
    /// The number of inputs of the router crossbar
    pub num_clients : usize,

    /// A reference to the Plugs object
    pub plugs : &'a Plugs,
    /// The random number generator to use
    pub rng : &'a RefCell<StdRng>,
}

pub fn new_allocator(arg:AllocatorBuilderArgument) -> Box<dyn Allocator>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        if let Some(builder) = arg.plugs.allocators.get(cv_name) {
            return builder(arg)
        };
        match cv_name.as_ref()
        {
            "Random" => Box::new(RandomAllocator::new(arg)),
            "RandomWithPriority" => Box::new(RandomPriorityAllocator::new(arg)),
            "Islip" => Box::new(IslipAllocator::new(arg)),
            _ => panic!("Unknown allocator: {}", cv_name),
        }
    }
    else
    {
        panic!("Trying to create an Allocator from a non-Object");
    }
}