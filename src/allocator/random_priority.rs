
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::prelude::SliceRandom;

//use quantifiable_derive::Quantifiable;//the derive macro
use crate::allocator::{Allocator, Request, GrantedRequests, AllocatorBuilderArgument};
use crate::config_parser::ConfigurationValue;
use crate::match_object_panic;


#[derive(Default, Clone)]
struct Resource {
    /// Index of the client that has the resource (or None if the resource is free)
    client: Option<usize>,
}

#[derive(Default, Clone)]
struct Client {
    /// Index of the resource that the client has (or None if the client has no resource)
    resource: Option<usize>,
}
/**
An allocator that allocates a resource to the request with the highest/lowest priority. Ties are solved randomly.
The priority of a request is the label assigned by the routing and policies of the router.
```
RandomPriorityAllocator{
	//seed:0
	//greatest_first:false
}
```

**/
pub struct RandomPriorityAllocator {
    /// The max number of outputs of the router crossbar
    num_resources: usize,
    /// The max number of inputs of the router crossbar
    num_clients: usize,
    /// The requests of the clients
    requests: Vec<Request>,
    /// Greatest first
    greatest_first: bool,
    /// The RNG or None if the seed is not set
    rng: Option<StdRng>,
}

impl RandomPriorityAllocator {
    /// Create a new random priority allocator
    /// # Parameters
    /// * `args` - The arguments for the allocator
    /// # Returns
    /// * `RandomPriorityAllocator` - The new random priority allocator
    pub fn new(args: AllocatorBuilderArgument) -> RandomPriorityAllocator {
        // Check if the arguments are valid
        if args.num_clients == 0 || args.num_resources == 0 {
            panic!("Invalid arguments")
        }
        // Get the seed from the configuration
        let mut seed = None;
        let mut greatest_first = false;
        match_object_panic!(args.cv, "RandomWithPriority", value,
			"seed" => match value
			{
				&ConfigurationValue::Number(s) => seed = Some(s as u64),
				_ => panic!("Bad value for seed"),
			},
			"greatest_first" => greatest_first = value.as_bool().expect("Bad value for greatest_first"),
        );
        let rng = seed.map(|s| StdRng::seed_from_u64(s));
        // Create the allocator
        RandomPriorityAllocator {
            num_resources: args.num_resources,
            num_clients: args.num_clients,
            requests: Vec::new(),
            greatest_first,
            rng,
        }
    }

    /// Check if the request is valid
    /// # Arguments
    /// * `request` - The request to check
    /// # Returns
    /// * `bool` - True if the request is valid, false otherwise
    /// # Remarks
    /// The request is valid if
    /// the client is in the range [0, num_clients) and
    /// the resource is in the range [0, num_resources) and
    /// the priority is is not None
    fn is_valid_request(&self, _request: &Request) -> bool {
        if _request.client >= self.num_clients || _request.resource >= self.num_resources || _request.priority.is_none() {
            return false
        }
        true
    }
}

impl Allocator for RandomPriorityAllocator {
    /// Add a request to the allocator
    /// # Arguments
    /// * `request` - The request to add
    /// # Remarks
    /// The request is valid if the client is in the range [0, num_clients) and the resource is in the range [0, num_resources) and the priority is is not None
    fn add_request(&mut self, request: Request) {
        // Check if the request is valid
        if !self.is_valid_request(&request) {
            panic!("Invalid request");
        }
        self.requests.push(request);
    }

    /// Perform the allocation
    /// # Arguments
    /// * `rng` - The RNG to use if the seed is not set
    /// # Returns
    /// * `GrantedRequests` - The granted requests
    /// # Remarks
    /// If the seed is not set, the passed RNG is used to generate the random numbers
    /// The granted requests are sorted by priority (from low to high)
    fn perform_allocation(&mut self, rng : &mut StdRng) -> GrantedRequests {
        // Create the granted requests vector
        let mut gr = GrantedRequests::default();
        
        // The resources allocated to the clients
        let mut resources: Vec<Resource> = vec![Resource::default(); self.num_resources];
        // The clients allocated to the resources
        let mut clients: Vec<Client> = vec![Client::default(); self.num_clients];


        // Shuffle the requests using the RNG passed as parameter
        // Except if the seed is set, in which case we use it
        let rng = self.rng.as_mut().unwrap_or(rng);
        self.requests.shuffle(rng);

        // Sort the requests by priority (least is first)
        self.requests.sort_by(|a, b| a.priority.unwrap().cmp(&b.priority.unwrap()));

        // Reverse the requests if greatest first is set
        if self.greatest_first {
            // Sort the requests by priority (greatest is first)
            self.requests.reverse();
        }

        // Allocate the requests with an iterator
        for Request{ref resource, ref client, ref priority } in self.requests.iter() {
            // Check if the wanted resource is available and the client has no resource
            if resources[*resource].client.is_none() && clients[*client].resource.is_none() {
                // Add the request to the granted requests
                gr.add_granted_request(Request{
                    client: *client,
                    resource: *resource,
                    priority: *priority,
                });
                // Allocate the resource
                resources[*resource].client = Some(*client);
                // Allocate the client
                clients[*client].resource = Some(*resource);
            } else {
                // The resource is not available or the client has a resource,
                // so we can't allocate the request
                continue;
            }
        }
        // Clear the requests vector
        self.requests.clear();
        // Return the granted requests
        gr
    }
    /// Check if the allocator supports the intransit priority option
    fn support_intransit_priority(&self) -> bool {
        true
    }
}

