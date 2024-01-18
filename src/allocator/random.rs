
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
/// A random allocator that randomly allocates requests to resources
pub struct RandomAllocator {
    /// The max number of outputs of the router crossbar
    num_resources: usize,
    /// The max number of inputs of the router crossbar
    num_clients: usize,
    /// The requests of the clients
    requests: Vec<Request>,
    /// The RNG or None if the seed is not set
    rng: Option<StdRng>,
}

impl RandomAllocator {
    /// Create a new random allocator
    /// # Parameters
    /// * `args` - The arguments for the allocator
    /// # Returns
    /// * `RandomAllocator` - The new random allocator
    pub fn new(args: AllocatorBuilderArgument) -> RandomAllocator {
        // Check if the arguments are valid
        if args.num_clients == 0 || args.num_resources == 0 {
            panic!("Invalid arguments")
        }
        // Get the seed from the configuration
        let mut rng = None;
        match_object_panic!(args.cv, "Random", value,
			"seed" => rng = Some(value.as_rng().expect("bad value for seed")),
        );
        // Create the allocator
        RandomAllocator {
            num_resources: args.num_resources,
            num_clients: args.num_clients,
            requests: Vec::new(),
            rng,
        }
    }

    /// Check if the request is valid
    /// # Arguments
    /// * `request` - The request to check
    /// # Returns
    /// * `bool` - True if the request is valid, false otherwise
    /// # Remarks
    /// The request is valid if the client is in the range [0, num_clients) and the resource is in the range [0, num_resources)
    fn is_valid_request(&self, _request: &Request) -> bool {
        if _request.client >= self.num_clients || _request.resource >= self.num_resources {
            return false
        }
        true
    }
}

impl Allocator for RandomAllocator {
    /// Add a request to the allocator
    /// # Arguments
    /// * `request` - The request to add
    /// # Remarks
    /// The request is valid if the client is in the range [0, num_clients) and the resource is in the range [0, num_resources)
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
    /// The granted requests are the requests that are granted
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
        // Allocate the requests with an iterator
        for Request{ref resource, ref client, priority: _ } in self.requests.iter() {
            // Check if the wanted resource is available and if the client has no resource
            if resources[*resource].client.is_none() && clients[*client].resource.is_none() {
                // Add the request to the granted requests
                gr.add_granted_request(Request{
                    client: *client,
                    resource: *resource,
                    priority: None, // Don't care about the priority on this allocator
                });
                // Allocate the resource to the client
                resources[*resource].client = Some(*client);
                // Allocate the client to the resource
                clients[*client].resource = Some(*resource);
            } else {
                // The resource or the client is not available, so we can't grant the request
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
        false
    }
}

