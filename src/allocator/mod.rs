/*!

An Allocator defines the interface for an allocation strategy for a router crossbar

see [`new_allocator`](fn.new_allocator.html) for documentation on the configuration syntax of predefined allocators.

*/

pub mod random;
pub mod random_priority;
pub mod islip;
mod label_reduction;
//pub mod separable_input_first;

use crate::Plugs;
use crate::config_parser::ConfigurationValue;

use ::rand::rngs::StdRng;
use random::RandomAllocator;
use random_priority::RandomPriorityAllocator;
use islip::ISLIPAllocator;


/// A request to a Virtual Channel Allocator.
/// A phit in the virtual channel `virtual_channel` of the port `entry_port` is requesting to go to the virtual channel `requested_vc` of the port `requested_port`.
/// The label is the one returned by the routing algorithm or 0 if that information has been lost. For example it is 0 after a conversion from a granted `Request` with no priority.
#[derive(Clone)]
pub struct VCARequest
{
	pub entry_port: usize,
	pub entry_vc: usize,
	pub requested_port: usize,
	pub requested_vc: usize,
	pub label: i32,
}

impl VCARequest
{
	// method to transform a VCARequest into a `allocator::Request`.
	pub fn to_allocator_request(&self, num_vcs: usize)->Request
	{
		Request::new(
			self.entry_port*num_vcs+self.entry_vc,
			self.requested_port*num_vcs+self.requested_vc,
			if self.label<0 {None} else {	Some(self.label as usize) },
		)
	}
}


/// A client (input of crossbar) want a resource (output of crossbar) with a certain priority.
/// The type structure with the Allocator work. Other request types, such as `VCARequest` have methods to be converted into a `Request`.
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
	pub fn to_port_request(&self, num_vcs: usize)->VCARequest
	{
		VCARequest{
			entry_port: self.client/num_vcs,
			entry_vc: self.client%num_vcs,
			requested_port: self.resource/num_vcs,
			requested_vc: self.resource%num_vcs,
			label: if self.priority.is_none() {0} else {self.priority.unwrap() as i32},
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

//impl Iterator for GrantedRequests {
//	  type Item = Request;
//	// TODO: This next is O(n) instead of O(1). Can it be causing a loss of performance?
//	  fn next(&mut self) -> Option<Self::Item> {
//		  if !self.granted_requests.is_empty() {
//			  let r = self.granted_requests.remove(0);
//			  Some(r)
//		  } else {
//			  None
//		  }
//	  }
//}

// This should be faster, but has not been verified.
impl IntoIterator for GrantedRequests {
	type Item = Request;
	type IntoIter = <Vec<Request> as IntoIterator>::IntoIter;
	fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
		self.granted_requests.into_iter()
	}
}

/**
An Allocator manages the requests from a set of clients to a set of resources. Requests are added via `add_request`.
When all requests have been made a call to `perform_allocation` returns a valid, possibly partial, allocation; its state is then cleared, removing all requests.

unrelated to `std::alloc::Allocator`.
**/
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
	fn perform_allocation(&mut self, rng : &mut StdRng) -> GrantedRequests;

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
	pub rng : &'a mut StdRng,
}

/**
The allocator `Random` fully randomizes all requests, ignoring priority. It is not clear whether it can be implemented, but helps to avoid specific details when a generic allocator is desired.
It avoids possible starvation pitfalls with other allocators. Note this is not a random separable-first allocator.
```ignore
Random{
	//Optional seed to build a new random generator independent of the simulation's global generator.
	//seed:42
}
```

The `RandomWithPriority` allocator is like the `Random` one, but sorts by priority.
```ignore
RandomWithPriority{
	//Optional seed to build a new random generator independent of the simulation's global generator.
	//seed:42
}
```

The well-known iSLIP allocator.
```ignore
Islip{
	//Number of iterations to perform.
	//Defaults to 1 if omitted.
	num_iter:2,
}
```
**/
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
			"LabelReduction" => Box::new(label_reduction::LabelReduction::new(arg)),
			"Islip" | "iSLIP" =>
			{
				let mut cv = arg.cv.clone();
				cv.rename("ISLIP".into());
				let alias = AllocatorBuilderArgument{cv:&cv,..arg};
				Box::new(ISLIPAllocator::new(alias))
			}
			"ISLIP" => Box::new(ISLIPAllocator::new(arg)),
			_ => panic!("Unknown allocator: {}", cv_name),
		}
	}
	else
	{
		panic!("Trying to create an Allocator from a non-Object");
	}
}
