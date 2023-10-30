/*!

A Pattern defines the way elements select their destinations.

see [`new_pattern`](fn.new_pattern.html) for documentation on the configuration syntax of predefined patterns.

*/

use std::cell::{RefCell};
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom,SeedableRng};
use std::fs::File;
use std::io::{BufRead,BufReader};

use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology,Location};
use crate::quantify::Quantifiable;
use crate::{Plugs,match_object_panic};

/// Some things most uses of the pattern module will use.
pub mod prelude
{
	pub use super::{Pattern,new_pattern,PatternBuilderArgument};
}

///A `Pattern` describes how a set of entities decides destinations into another set of entities.
///The entities are initially servers, but after some operators it may mean router, rows/columns, or other agrupations.
///The source and target set may be or not be the same. Or even be of different size.
///Thus, a `Pattern` is a generalization of the mathematical concept of function.
pub trait Pattern : Quantifiable + std::fmt::Debug
{
	//Indices are either servers or virtual things.
	///Fix the input and output size, providing the topology and random number generator.
	///Careful with using toology in sub-patterns. For example, it may be misleading to use the dragonfly topology when
	///building a pattern among groups or a pattern among the ruters of a single group.
	///Even just a pattern of routers instead of a pattern of servers can lead to mistakes.
	///Read the documentation of the traffic or meta-pattern using the pattern to know what its their input and output.
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng);
	///Obtain a destination of a source. This will be called repeteadly as the traffic requires destination for its messages.
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize;
}

///The argument to a builder funtion of patterns.
#[derive(Debug)]
pub struct PatternBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the pattern.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the pattern needs to create elements.
	pub plugs: &'a Plugs,
}


/**Build a new pattern. Patterns are maps between two sets which may depend on the RNG. Generally over the whole set of servers, but sometimes among routers or groups. Check the domentation of the parent Traffic/Permutation for its interpretation.

## Roughly uniform patterns

### Uniform

In the [uniform](UniformPattern) pattern all elements have same probability to send to any other.
```ignore
Uniform{
	legend_name: "uniform",
}
```

### GloballyShufflingDestinations

The [GloballyShufflingDestinations] is an uniform-like pattern that avoids repeating the same destination. It keeps a global vector of destinations. It is shuffled and each created message gets its destination from there. Sometimes you may be selected yourself as destination.

```ignore
GloballyShufflingDestinations{
	legend_name: "globally shuffled destinations",
}
```

### GroupShufflingDestinations

The [GroupShufflingDestinations] pattern is alike [GloballyShufflingDestinations] but keeping one destination vector per each group.

```ignore
GroupShufflingDestinations{
	//E.g., if we select `group_size` to be the number of servers per router we are keeping a destination vector for each router.
	group_size: 5,
	legend_name: "router shuffled destinations",
}
```

### UniformDistance

In [UniformDistance] each message gets its destination sampled uniformly at random among the servers attached to neighbour routers.
It may build a pattern either of servers or switches, controlled through the `switch_level` configuration flag.
This pattern autoscales if requested a size multiple of the network size.

Example configuration:
```ignore
UniformDistance{
	///The distance at which the destination must be from the source.
	distance: 1,
	/// Optionally build the pattern at the switches. This should be irrelevant at direct network with the same number of servers per switch.
	//switch_level: true,
	legend_name: "uniform among neighbours",
}
```

### RestrictedMiddleUniform
[RestrictedMiddleUniform] is a pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying some criteria. Note this is only a pattern, the actual packet route does not have to go throught such middle router.
It has the same implicit concentration scaling as UniformDistance, allowing building a pattern over a multiple of the number of switches.

Example configuration:
```ignore
RestrictedMiddleUniform{
	/// An optional integer value to allow only middle routers whose index is greater or equal to it.
	minimum_index: 100,
	/// An optional integer value to allow only middle routers whose index is lower or equal to it.
	// maximum_index: 100,
	/// Optionally, give a vector with the possible values of the distance from the source to the middle.
	distances_to_source: [1],
	/// Optionally, give a vector with the possible values of the distance from the middle to the destination.
	distances_to_destination: [1],
	/// Optionally, a vector with distances from source to destination, ignoring middle.
	distances_source_to_destination: [2],
	/// Optionally, set a pattern for those sources with no legal destination.
	else: Uniform,
}
```

## Permutations and maps.
Each element has a unique destination and a unique element from which it is a destination.

### RandomPermutation
The [RandomPermutation] has same chance to generate any permutation
```ignore
RandomPermutation{
	legend_name: "random server permutation",
}
```

### RandomInvolution
The [RandomInvolution] can only generate involutions. This is, if `p` is the permutation then for any element `x`, `p(p(x))=x`.
```ignore
RandomInvolution{
	legend_name: "random server involution",
}
```

### FixedRandom
In [FixedRandom] each source has an independent unique destination. By the "birthday paradox" we can expect several sources to share a destination, causing incast contention.

### FileMap
With [FileMap] a map is read from a file. Each elment has a unique destination.
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```

### CartesianTransform
With [CartesianTransform] the nodes are seen as in a n-dimensional orthohedra. Then it applies several transformations. When mapping directly servers it may be useful to use as `sides[0]` the number of servers per router.
```ignore
CartesianTransform{
	sides: [4,8,8],
	multiplier: [1,1,1],//optional
	shift: [0,4,0],//optional
	permute: [0,2,1],//optional
	complement: [false,true,false],//optional
	project: [false,false,false],//optional
	//random: [false,false,true],//optional
	//patterns: [Identity,Identity,Circulant{generators:[1,-1]}]//optional
	legend_name: "Some lineal transformation over a 8x8 mesh with 4 servers per router",
}
```

### Hotspots
[Hotspots] builds a pool of hotspots from a given list of `destinations` plus some amount `extra_random_destinations` computed randomly on initialization.
Destinations are randomly selected from such pool.
This causes incast contention, more explicitly than `FixedRandom`.
```ignore
Hotspots{
	//destinations: [],//default empty
	extra_random_destinations: 5,//default 0
	legend_name: "every server send to one of 5 randomly selected hotspots",
}
```

### Circulant
In [Circulant] each node send traffic to the node `current+g`, where `g` is any of the elements given in the vector `generators`. The operations
being made modulo the destination size. Among the candidates one of them is selected in each call with uniform distribution.

In this example each node `x` send to either `x+1` or `x+2`.
```ignore
Circulant{
	generators: [1,2],
}
```

### CartesianEmbedding

[CartesianEmbedding] builds the natural embedding between two blocks, by keeping the coordinate.

Example mapping nodes in a block of 16 nodes into one of 64 nodes.
```ignore
CartesianEmbedding{
	source_sides: [4,4],
	destination_sides: [8,8],
}
```

## meta patterns

### Product
With [Product](ProductPattern) the elements are divided in blocks. Blocks are mapped to blocks by the `global_pattern`. The `block_pattern` must has input and output size equal to `block_size` and maps the specific elements.
```ignore
Product{
	block_pattern: RandomPermutation,
	global_pattern: RandomPermutation,
	block_size: 10,
	legend_name:"permutation of blocks",
}
```

### Components
[Components](ComponentsPattern) divides the topology along link classes. The 'local' pattern is Uniform.
```ignore
Components{
	global_pattern: RandomPermutation,
	component_classes: [0],
	legend_name: "permutation of the induced group by the 0 link class",
}
```

### Composition
The [Composition] pattern allows to concatenate transformations.
```ignore
Composition{
	patterns: [  FileMap{filename: "/patterns/second"}, FileMap{filename: "/patterns/first"}  ]
	legend_name: "Apply first to origin, and then second to get the destination",
}
```


### Pow
A [Pow] is composition of a `pattern` with itself `exponent` times.
```ignore
Pow{
	pattern: FileMap{filename: "/patterns/mypattern"},
	exponent: "3",
	legend_name: "Apply 3 times my pattern",
}
```


### RandomMix
[RandomMix] probabilistically mixes a list of patterns.
```ignore
RandomMix{
	patterns: [Hotspots{extra_random_destinations:10}, Uniform],
	weights: [5,95],
	legend_name: "0.05 chance of sending to the hotspots",
}
```

### IndependentRegions
With [IndependentRegions] the set of nodes is partitioned in independent regions, each with its own pattern. Source and target sizes must be equal.
```ignore
IndependentRegions{
	// An array with the patterns for each region.
	patterns: [Uniform, Hotspots{destinations:[0]}],
	// An array with the size of each region. They must add up to the total size.
	sizes: [100, 50],
	// Alternatively, use relative_sizes. the pattern will be initialized with sizes proportional to these.
	// You must use exactly one of either `sizes` or `relative_sizes`.
	// relative_sizes: [88, 11],
}
```
### RemappedNodes
[RemappedNodes] allows to apply another pattern using indices that are mapped by another pattern.

Example building a cycle in random order.
```ignore
RemappedNodes{
	/// The underlaying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

*/
pub fn new_pattern(arg:PatternBuilderArgument) -> Box<dyn Pattern>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		if let Some(builder) = arg.plugs.patterns.get(cv_name)
		{
			return builder(arg);
		}
		match cv_name.as_ref()
		{
			"Identity" => Box::new(Identity::new(arg)),
			"Uniform" => Box::new(UniformPattern::new(arg)),
			"RandomPermutation" => Box::new(RandomPermutation::new(arg)),
			"RandomInvolution" => Box::new(RandomInvolution::new(arg)),
			"FileMap" => Box::new(FileMap::new(arg)),
			"EmbeddedMap" => Box::new(FileMap::embedded(arg)),
			"Product" => Box::new(ProductPattern::new(arg)),
			"Components" => Box::new(ComponentsPattern::new(arg)),
			"CartesianTransform" => Box::new(CartesianTransform::new(arg)),
			"CartesianTiling" => Box::new(CartesianTiling::new(arg)),
			"Composition" => Box::new(Composition::new(arg)),
			"Pow" => Box::new(Pow::new(arg)),
			"CartesianFactor" => Box::new(CartesianFactor::new(arg)),
			"CartesianFactorDimension" => Box::new(CartesianFactorDimension::new(arg)),
			"Hotspots" => Box::new(Hotspots::new(arg)),
			"RandomMix" => Box::new(RandomMix::new(arg)),
			"ConstantShuffle" =>
			{
				println!("WARNING: the name ConstantShuffle is deprecated, use GloballyShufflingDestinations");
				Box::new(GloballyShufflingDestinations::new(arg))
			}
			"GloballyShufflingDestinations" => Box::new(GloballyShufflingDestinations::new(arg)),
			"GroupShufflingDestinations" => Box::new(GroupShufflingDestinations::new(arg)),
			"UniformDistance" => Box::new(UniformDistance::new(arg)),
			"FixedRandom" => Box::new(FixedRandom::new(arg)),
			"IndependentRegions" => Box::new(IndependentRegions::new(arg)),
			"RestrictedMiddleUniform" => Box::new(RestrictedMiddleUniform::new(arg)),
			"Circulant" => Box::new(Circulant::new(arg)),
			"CartesianEmbedding" => Box::new(CartesianEmbedding::new(arg)),
			"RemappedNodes" => Box::new(RemappedNodes::new(arg)),
			_ => panic!("Unknown pattern {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a Pattern from a non-Object");
	}
}

///Just set `destination = origin`.
///Mostly to be used inside some meta-patterns.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Identity
{
}

impl Pattern for Identity
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!=target_size
		{
			unimplemented!("The Identity pattern requires source_size({})=target_size({})",source_size,target_size);
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		origin
	}
}

impl Identity
{
	fn new(arg:PatternBuilderArgument) -> Identity
	{
		match_object_panic!(arg.cv,"Identity",_value);
		Identity{
		}
	}
}

///Each destination request will be uniform random among all the range `0..size` minus the `origin`.
///Independently of past requests, decisions or origin.
///Has an optional configuration argument `allow_self`, default to false.
///This can be useful for composed patterns, for example, for a group to send uniformly into another group.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformPattern
{
	size: usize,
	allow_self: bool,
}

impl Pattern for UniformPattern
{
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.size=target_size;
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let discard_self = !self.allow_self && origin<self.size;
		let random_size = if discard_self { self.size-1 } else { self.size };
		// When discard self, act like self were swapped with the last element.
		// If it were already the last element it is already outside the random range.
		let r=rng.gen_range(0..random_size);
		if discard_self && r==origin {
			random_size
		} else {
			r
		}
	}
}

impl UniformPattern
{
	fn new(arg:PatternBuilderArgument) -> UniformPattern
	{
		let mut allow_self = false;
		match_object_panic!(arg.cv,"Uniform",value,
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
		UniformPattern{
			size:0,//to be initialized later
			allow_self,
		}
	}
	pub fn uniform_pattern(allow_target_source: bool) -> UniformPattern
	{
		UniformPattern{
			size:0,//to be initialized later
			allow_self:allow_target_source,
		}
	}
}

///Build a random permutation on initialization, which is then kept constant.
///This allows self-messages; with a reasonable probability of having one.
///See `RandomInvolution` and `FileMap`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomPermutation
{
	permutation: Vec<usize>,
}

impl Pattern for RandomPermutation
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
	{
		if source_size!=target_size
		{
			panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		self.permutation=(0..source_size).collect();
		self.permutation.shuffle(rng);
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		self.permutation[origin]
	}
}

impl RandomPermutation
{
	fn new(arg:PatternBuilderArgument) -> RandomPermutation
	{
		match_object_panic!(arg.cv,"RandomPermutation",_value);
		RandomPermutation{
			permutation: vec![],
		}
	}
}

///Build a random involution on initialization, which is then kept constant.
///An involution is a permutation that is a pairing/matching; if `a` is the destination of `b` then `b` is the destination of `a`.
///It will panic if given an odd size.
///See `Randompermutation`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomInvolution
{
	permutation: Vec<usize>,
}

impl Pattern for RandomInvolution
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
	{
		if source_size!=target_size
		{
			panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		//self.permutation=(0..source_size).collect();
		//rng.shuffle(&mut self.permutation);
		self.permutation=vec![source_size;source_size];
		//for index in 0..source_size
		//{
		//	if self.permutation[index]==source_size
		//	{
		//		//Look for a partner
		//	}
		//}
		assert!(source_size%2==0);
		//Todo: annotate this weird algotihm.
		let iterations=source_size/2;
		let mut max=2;
		for _iteration in 0..iterations
		{
			let first=rng.gen_range(0..max);
			let second=rng.gen_range(0..max-1);
			let (low,high) = if second>=first
			{
				(first,second+1)
			}
			else
			{
				(second,first)
			};
			let mut rep_low = max-2;
			let mut rep_high = max-1;
			if high==rep_low
			{
				rep_high=high;
				rep_low=max-1;
			}
			let mut mate_low=self.permutation[low];
			let mut mate_high=self.permutation[high];
			if mate_low != source_size
			{
				if mate_low==high
				{
					mate_low=rep_high;
				}
				self.permutation[rep_low]=mate_low;
				self.permutation[mate_low]=rep_low;
			}
			if mate_high != source_size
			{
				if mate_high==low
				{
					mate_high=rep_low;
				}
				self.permutation[rep_high]=mate_high;
				self.permutation[mate_high]=rep_high;
			}
			self.permutation[low]=high;
			self.permutation[high]=low;
			max+=2;
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		self.permutation[origin]
	}
}

impl RandomInvolution
{
	fn new(arg:PatternBuilderArgument) -> RandomInvolution
	{
		match_object_panic!(arg.cv,"RandomInvolution",_value);
		RandomInvolution{
			permutation: vec![],
		}
	}
}


/**
A map read from file. Each node has a unique destination. See [RandomPermutation] for related matters.
The file is read at creation and should contain only lines with pairs `source destination`.

Example configuration:
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FileMap
{
	permutation: Vec<usize>,
}

impl Pattern for FileMap
{
	fn initialize(&mut self, _source_size:usize, _target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		//self.permutation=(0..size).collect();
		//rng.shuffle(&mut self.permutation);
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		self.permutation[origin]
	}
}

impl FileMap
{
	fn new(arg:PatternBuilderArgument) -> FileMap
	{
		let mut filename=None;
		match_object_panic!(arg.cv,"FileMap",value,
			"filename" => filename = Some(value.as_str().expect("bad value for filename").to_string()),
		);
		let filename=filename.expect("There were no filename");
		let file=File::open(&filename).expect("could not open pattern file.");
		let reader = BufReader::new(&file);
		let mut permutation=Vec::new();
		for rline in reader.lines()
		{
			let line=rline.expect("Some problem when reading the traffic pattern.");
			let mut words=line.split_whitespace();
			let origin=words.next().unwrap().parse::<usize>().unwrap();
			let destination=words.next().unwrap().parse::<usize>().unwrap();
			while permutation.len()<=origin || permutation.len()<=destination
			{
				permutation.push((-1isize) as usize);//which value use as filler?
			}
			permutation[origin]=destination;
		}
		FileMap{
			permutation,
		}
	}
	fn embedded(arg:PatternBuilderArgument) -> FileMap
	{
		let mut map = None;
		match_object_panic!(arg.cv,"EmbeddedMap",value,
			"map" => map = Some(value.as_array()
				.expect("bad value for map").iter()
				.map(|v|v.as_f64().expect("bad value for map") as usize).collect()),
		);
		let permutation = map.expect("There were no map");
		FileMap{
			permutation
		}
	}
}

///A pattern given by blocks. The elements are divided by blocks of size `block_size`. The `global_pattern` is used to describe the communication among different blocks and the `block_pattern` to describe the communication inside a block.
///Seen as a graph, this is the Kronecker product of the block graph with the global graph.
///Thus the origin a position `i` in the block `j` will select the destination at position `b(i)` in the block `g(j)`, where `b(i)` is the destination via the `block_pattern` and `g(j)` is the destination via the `global_pattern`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ProductPattern
{
	block_size: usize,
	block_pattern: Box<dyn Pattern>,
	global_pattern: Box<dyn Pattern>,
}

impl Pattern for ProductPattern
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		if source_size!=target_size
		{
			unimplemented!("Different sizes are not yet implemented for ProductPattern");
		}
		self.block_pattern.initialize(self.block_size,self.block_size,topology,rng);
		let global_size=source_size/self.block_size;
		self.global_pattern.initialize(global_size,global_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let local=origin % self.block_size;
		let global=origin / self.block_size;
		let local_dest=self.block_pattern.get_destination(local,topology,rng);
		let global_dest=self.global_pattern.get_destination(global,topology,rng);
		global_dest*self.block_size+local_dest
	}
}

impl ProductPattern
{
	fn new(arg:PatternBuilderArgument) -> ProductPattern
	{
		let mut block_size=None;
		let mut block_pattern=None;
		let mut global_pattern=None;
		match_object_panic!(arg.cv,"Product",value,
			"block_pattern" => block_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"block_size" => block_size=Some(value.as_f64().expect("bad value for block_size") as usize),
		);
		let block_size=block_size.expect("There were no block_size");
		let block_pattern=block_pattern.expect("There were no block_pattern");
		let global_pattern=global_pattern.expect("There were no global_pattern");
		ProductPattern{
			block_size,
			block_pattern,
			global_pattern,
		}
	}
}

///Divide the topology according to some given link classes, considering the graph components if the other links were removed.
///Then apply the `global_pattern` among the components and select randomly inside the destination comonent.
///Note that this uses the topology and will cause problems if used as a sub-pattern.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ComponentsPattern
{
	component_classes: Vec<usize>,
	//block_pattern: Box<dyn Pattern>,//we would need patterns between places of different extent.
	global_pattern: Box<dyn Pattern>,
	components: Vec<Vec<usize>>,
}

impl Pattern for ComponentsPattern
{
	fn initialize(&mut self, _source_size:usize, _target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		let mut allowed_components=vec![];
		for link_class in self.component_classes.iter()
		{
			if *link_class>=allowed_components.len()
			{
				allowed_components.resize(*link_class+1,false);
			}
			allowed_components[*link_class]=true;
		}
		self.components=topology.components(&allowed_components);
		//for (i,component) in self.components.iter().enumerate()
		//{
		//	println!("component {}: {:?}",i,component);
		//}
		self.global_pattern.initialize(self.components.len(),self.components.len(),topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		//let local=origin % self.block_size;
		//let global=origin / self.block_size;
		//let n=topology.num_routers();
		let router_origin=match topology.server_neighbour(origin).0
		{
			Location::RouterPort{
				router_index,
				router_port: _,
			} => router_index,
			_ => panic!("what origin?"),
		};
		let mut global=self.components.len();
		for (g,component) in self.components.iter().enumerate()
		{
			if component.contains(&router_origin)
			{
				global=g;
				break;
			}
		}
		if global==self.components.len()
		{
			panic!("Could not found component of {}",router_origin);
		}
		let global_dest=self.global_pattern.get_destination(global,topology,rng);
		//let local_dest=self.block_pattern.get_destination(local,topology,rng);
		let r_local=rng.gen_range(0..self.components[global_dest].len());
		let dest=self.components[global_dest][r_local];
		let radix=topology.ports(dest);
		let mut candidate_stack=Vec::with_capacity(radix);
		for port in 0..radix
		{
			match topology.neighbour(dest,port).0
			{
				Location::ServerPort(destination) => candidate_stack.push(destination),
				_ => (),
			}
		}
		let rserver=rng.gen_range(0..candidate_stack.len());
		candidate_stack[rserver]
	}
}

impl ComponentsPattern
{
	fn new(arg:PatternBuilderArgument) -> ComponentsPattern
	{
		let mut component_classes=None;
		//let mut block_pattern=None;
		let mut global_pattern=None;
		match_object_panic!(arg.cv,"Components",value,
			"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"component_classes" => component_classes = Some(value.as_array()
				.expect("bad value for component_classes").iter()
				.map(|v|v.as_f64().expect("bad value in component_classes") as usize).collect()),
		);
		let component_classes=component_classes.expect("There were no component_classes");
		//let block_pattern=block_pattern.expect("There were no block_pattern");
		let global_pattern=global_pattern.expect("There were no global_pattern");
		ComponentsPattern{
			component_classes,
			//block_pattern,
			global_pattern,
			components:vec![],//filled at initialize
		}
	}
}


/**
Interpretate the origin as with cartesian coordinates and apply transformations.
May permute the dimensions if they have same side.
May complement the dimensions.
Order of composition is: multiplier, shift, permute, complement, project, randomize, pattern. If you need another order you may [compose](Composition) several of them.

Example configuration:
```ignore
CartesianTransform{
	sides: [4,8,8],
	multiplier: [1,1,1],//optional
	shift: [0,4,0],//optional
	permute: [0,2,1],//optional
	complement: [false,true,false],//optional
	project: [false,false,false],//optional
	//random: [false,false,true],//optional
	//patterns: [Identity,Identity,Circulant{generators:[1,-1]}]//optional
	legend_name: "Some lineal transformation over a 8x8 mesh with 4 servers per router",
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianTransform
{
	///The Cartesian interpretation.
	cartesian_data: CartesianData,
	///A factor multiplying each coordinate. Use 1 for nops.
	multiplier: Option<Vec<i32>>,
	///A shift to each coordinate, modulo the side. Use 0 for nops.
	shift: Option<Vec<usize>>,
	///Optionally how dimensions are permuted.
	///`permute=[0,2,1]` means to permute dimensions 1 and 2, keeping dimension 0 as is.
	permute: Option<Vec<usize>>,
	///Optionally, which dimensions must be complemented.
	///`complement=[true,false,false]` means `target_coordinates[0]=side-1-coordinates[0]`.
	complement: Option<Vec<bool>>,
	///Indicates dimensions to be projected into 0. This causes incast contention.
	project: Option<Vec<bool>>,
	///Indicates dimensions in which to select a random coordinate.
	///A random roll performed in each call to `get_destination`.
	random: Option<Vec<bool>>,
	///Optionally, set a pattern at coordinate. Use Identity for those coordinates with no operation.
	patterns: Option<Vec<Box<dyn Pattern>>>,
}

impl Pattern for CartesianTransform
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		if source_size!=target_size
		{
			panic!("In a Cartesiantransform source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		if source_size!=self.cartesian_data.size
		{
			panic!("Sizes do not agree on CartesianTransform.");
		}
		if let Some(ref mut patterns) = self.patterns
		{
			for (index,ref mut pat) in patterns.iter_mut().enumerate()
			{
				let coordinate_size = self.cartesian_data.sides[index];
				pat.initialize(coordinate_size, coordinate_size, topology, rng );
			}
		}
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		use std::convert::TryInto;
		let up_origin=self.cartesian_data.unpack(origin);
		let up_multiplied=match self.multiplier
		{
			Some(ref v) => v.iter().enumerate().map(|(index,&value)|{
				let dst:i32  = (up_origin[index] as i32*value).rem_euclid(self.cartesian_data.sides[index] as i32);
				dst.try_into().unwrap()
			}).collect(),
			None => up_origin,
		};
		let up_shifted=match self.shift
		{
			Some(ref v) => v.iter().enumerate().map(|(index,&value)|(up_multiplied[index]+value)%self.cartesian_data.sides[index]).collect(),
			None => up_multiplied,
		};
		let up_permuted=match self.permute
		{
			//XXX Should we panic on side mismatch?
			Some(ref v) => v.iter().map(|&index|up_shifted[index]).collect(),
			None => up_shifted,
		};
		let up_complemented=match self.complement
		{
			Some(ref v) => up_permuted.iter().enumerate().map(|(index,&value)|if v[index]{self.cartesian_data.sides[index]-1-value}else {value}).collect(),
			None => up_permuted,
		};
		let up_projected=match self.project
		{
			Some(ref v) => up_complemented.iter().enumerate().map(|(index,&value)|if v[index]{0} else {value}).collect(),
			None => up_complemented,
		};
		let up_randomized=match self.random
		{
			Some(ref v) => up_projected.iter().enumerate().map(|(index,&value)|if v[index]{rng.gen_range(0..self.cartesian_data.sides[index])} else {value}).collect(),
			None => up_projected,
		};
		let up_patterned = match self.patterns
		{
			Some(ref v) => up_randomized.iter().enumerate().map(|(index,&value)|v[index].get_destination(value,topology,rng)).collect(),
			None => up_randomized,
		};
		self.cartesian_data.pack(&up_patterned)
	}
}

impl CartesianTransform
{
	fn new(arg:PatternBuilderArgument) -> CartesianTransform
	{
		let mut sides:Option<Vec<_>>=None;
		let mut shift=None;
		let mut multiplier=None;
		let mut permute=None;
		let mut complement=None;
		let mut project=None;
		let mut random =None;
		let mut patterns=None;
		match_object_panic!(arg.cv,"CartesianTransform",value,
			"sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"multiplier" => multiplier=Some(value.as_array().expect("bad value for multiplier").iter()
				.map(|v|v.as_i32().expect("bad value in multiplier") ).collect()),
			"shift" => shift=Some(value.as_array().expect("bad value for shift").iter()
				.map(|v|v.as_usize().expect("bad value in shift") ).collect()),
			"permute" => permute=Some(value.as_array().expect("bad value for permute").iter()
				.map(|v|v.as_usize().expect("bad value in permute") ).collect()),
			"complement" => complement=Some(value.as_array().expect("bad value for complement").iter()
				.map(|v|v.as_bool().expect("bad value in complement")).collect()),
			"project" => project=Some(value.as_array().expect("bad value for project").iter()
				.map(|v|v.as_bool().expect("bad value in project")).collect()),
			"random" => random=Some(value.as_array().expect("bad value for random").iter()
				.map(|v|v.as_bool().expect("bad value in random")).collect()),
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
		);
		let sides=sides.expect("There were no sides");
		//let permute=permute.expect("There were no permute");
		//let complement=complement.expect("There were no complement");
		CartesianTransform{
			cartesian_data: CartesianData::new(&sides),
			multiplier,
			shift,
			permute,
			complement,
			project,
			random,
			patterns,
		}
	}
}


/// Extend a pattern by giving it a Cartesian representation and a number of repetition periods per dimension.
/// E.g., it may translate a permutation on a 4x4 mesh into a 16x16 mesh.
/// Or it may translate a permutation of routers of a 4x2x2 mesh into a server permutation of 8x8x8x8 by using `[8,2,4,4]` as repetitions.
#[derive(Quantifiable)]
#[derive(Debug)]
struct CartesianTiling
{
	/// The original pattern.
	pattern: Box<dyn Pattern>,
	/// The Cartesian interpretation of the original pattern.
	base_cartesian_data: CartesianData,
	/// How much to repeat at each dimension.
	repetitions: Vec<usize>,
	/// The final Cartesian representation.
	final_cartesian_data: CartesianData,
}

impl Pattern for CartesianTiling
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		let factor: usize = self.repetitions.iter().product();
		assert!(source_size % factor == 0);
		assert!(target_size % factor == 0);
		let base_source_size = source_size / factor;
		let base_target_size = target_size / factor;
		self.pattern.initialize(base_source_size,base_target_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let up_origin=self.final_cartesian_data.unpack(origin);
		let n=up_origin.len();
		let base_up_origin:Vec<usize> = (0..n).map(|index|up_origin[index]%self.base_cartesian_data.sides[index]).collect();
		let base_origin = self.base_cartesian_data.pack(&base_up_origin);
		let base_destination = self.pattern.get_destination(base_origin,topology,rng);
		let base_up_destination = self.base_cartesian_data.unpack(base_destination);
		let up_destination:Vec<usize> = (0..n).map(|index|{
			let size = self.base_cartesian_data.sides[index];
			let tile = up_origin[index]/size;
			base_up_destination[index] + size*tile
		}).collect();
		self.final_cartesian_data.pack(&up_destination)
	}
}

impl CartesianTiling
{
	pub fn new(arg:PatternBuilderArgument) -> CartesianTiling
	{
		let mut pattern = None;
		let mut sides:Option<Vec<_>>=None;
		let mut repetitions:Option<Vec<_>> = None;
		match_object_panic!(arg.cv,"CartesianTiling",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_f64().expect("bad value in sides") as usize).collect()),
			"repetitions" => repetitions = Some(value.as_array().expect("bad value for repetitions").iter()
				.map(|v|v.as_f64().expect("bad value in repetitions") as usize).collect()),
		);
		let pattern=pattern.expect("There were no pattern");
		let sides=sides.expect("There were no sides");
		let repetitions=repetitions.expect("There were no repetitions");
		let n=sides.len();
		assert!(n==repetitions.len());
		let final_sides : Vec<_> = (0..n).map(|index|sides[index]*repetitions[index]).collect();
		CartesianTiling{
			pattern,
			base_cartesian_data: CartesianData::new(&sides),
			repetitions,
			final_cartesian_data: CartesianData::new(&final_sides),
		}
	}
}


///The pattern resulting of composing a list of patterns.
///`destination=patterns[len-1]( patterns[len-2] ( ... (patterns[1] ( patterns[0]( origin ) )) ) )`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Composition
{
	patterns: Vec<Box<dyn Pattern>>,
}

impl Pattern for Composition
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		for pattern in self.patterns.iter_mut()
		{
			pattern.initialize(source_size,target_size,topology,rng);
		}
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut destination=origin;
		for pattern in self.patterns.iter()
		{
			destination=pattern.get_destination(destination,topology,rng);
		}
		destination
	}
}

impl Composition
{
	fn new(arg:PatternBuilderArgument) -> Composition
	{
		let mut patterns=None;
		match_object_panic!(arg.cv,"Composition",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
		);
		let patterns=patterns.expect("There were no patterns");
		Composition{
			patterns,
		}
	}
}



///The pattern resulting of composing a pattern with itself a number of times..
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Pow
{
	pattern: Box<dyn Pattern>,
	exponent: usize,
}

impl Pattern for Pow
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.pattern.initialize(source_size,target_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut destination=origin;
		for _ in 0..self.exponent
		{
			destination=self.pattern.get_destination(destination,topology,rng);
		}
		destination
	}
}

impl Pow
{
	fn new(arg:PatternBuilderArgument) -> Pow
	{
		let mut pattern=None;
		let mut exponent=None;
		match_object_panic!(arg.cv,"Pow",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"exponent" => exponent=Some(value.as_f64().expect("bad value for exponent") as usize),
		);
		let pattern=pattern.expect("There were no pattern");
		let exponent=exponent.expect("There were no exponent");
		Pow{
			pattern,
			exponent,
		}
	}
}


/// Interpretate the origin as with cartesian coordinates. Then add each coordinate with a given factor.
/// It uses default `f64 as usize`, so a small epsilon may be desired.
/// We do not restrict the destination size to be equal to the source size.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianFactor
{
	///The Cartesian interpretation.
	cartesian_data: CartesianData,
	///The coefficient by which it is multiplied each dimension.
	factors: Vec<f64>,
	///As given in initialization.
	target_size: usize,
}

impl Pattern for CartesianFactor
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.target_size = target_size;
		if source_size!=self.cartesian_data.size
		{
			panic!("Sizes do not agree on CartesianFactor.");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		let up_origin=self.cartesian_data.unpack(origin);
		let destination = up_origin.iter().zip(self.factors.iter()).map(|(&coord,&f)|coord as f64 * f).sum::<f64>() as usize;
		destination % self.target_size
	}
}

impl CartesianFactor
{
	fn new(arg:PatternBuilderArgument) -> CartesianFactor
	{
		let mut sides: Option<Vec<_>>=None;
		let mut factors=None;
		match_object_panic!(arg.cv,"CartesianFactor",value,
			"sides" => sides=Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_f64().expect("bad value in sides") as usize).collect()),
			"factors" => factors=Some(value.as_array().expect("bad value for factors").iter()
				.map(|v|v.as_f64().expect("bad value in factors")).collect()),
		);
		let sides=sides.expect("There were no sides");
		let factors=factors.expect("There were no factors");
		CartesianFactor{
			cartesian_data: CartesianData::new(&sides),
			factors,
			target_size:0,
		}
	}
}


/// Interpretate the origin as with cartesian coordinates. Multiply the first coordinate with a given factor
/// and divide it by each dimension size until it is smaller than the dimension size of a dimension.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianFactorDimension
{
	///The Cartesian interpretation.
	cartesian_data: CartesianData,
	///The coefficient by which it is multiplied each dimension.
	factor: usize,
	///As given in initialization.
	target_size: usize,
	///The coefficient by which it is multiplied each dimension.
	factors: Vec<f64>,
}

impl Pattern for CartesianFactorDimension
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.target_size = target_size;
		if source_size!=self.cartesian_data.size
		{
			panic!("Sizes do not agree on CartesianFactorDimension.");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		let mut up_origin=self.cartesian_data.unpack(origin);
		let mut factor = self.factor * up_origin[0];

		for f in 0..up_origin.len()
		{
			if factor < self.cartesian_data.sides[f]
			{
				up_origin[f] = (up_origin[f]+ factor) % self.cartesian_data.sides[f];
				break;
			}
			factor = (factor / self.cartesian_data.sides[f]) as usize;
		}
		let destination = self.cartesian_data.pack(&up_origin); //.iter().zip(self.factors.iter()).map(|(&coord,&f)|coord as f64 * f).sum::<f64>() as usize;


		//println!("origin: {}, destination: {}", origin, destination);
        destination// % self.target_size
	}
}

impl CartesianFactorDimension
{
	fn new(arg:PatternBuilderArgument) -> CartesianFactorDimension
	{
		let mut sides: Option<Vec<_>>=None;
		let mut factor=None;
		let mut factors=None;

		match_object_panic!(arg.cv,"CartesianFactorDimension",value,
			"sides" => sides=Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_f64().expect("bad value in sides") as usize).collect()),
			"factor" => factor=Some(value.as_f64().expect("bad value for factor") as usize),
			"factors" => factors=Some(value.as_array().expect("bad value for factors").iter()
				.map(|v|v.as_f64().expect("bad value in factors")).collect()),

		);
		let sides=sides.expect("There were no sides");
		let factors=factors.expect("There were no factors");
		let factor=factor.expect("There were no factor");

		CartesianFactorDimension{
			cartesian_data: CartesianData::new(&sides),
			factor,
			target_size:0,
			factors
		}
	}
}


/// The destinations are selected from a given pool of servers.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Hotspots
{
	///The allowed destinations
	destinations: Vec<usize>,
	///An amount of destinations o be added to the vector on pattern initialization.
	extra_random_destinations: usize
}

impl Pattern for Hotspots
{
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
	{
		//XXX Do we want to check the user given destinations against target_size?
		for _ in 0..self.extra_random_destinations
		{
			let r=rng.gen_range(0..target_size);
			self.destinations.push(r);
		}
		if self.destinations.is_empty()
		{
			panic!("The Hotspots pattern requires to have at least one destination.");
		}
	}
	fn get_destination(&self, _origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let r = rng.gen_range(0..self.destinations.len());
		self.destinations[r]
	}
}

impl Hotspots
{
	fn new(arg:PatternBuilderArgument) -> Hotspots
	{
		let mut destinations=None;
		let mut extra_random_destinations=None;
		match_object_panic!(arg.cv,"Hotspots",value,
			"destinations" => destinations=Some(value.as_array().expect("bad value for destinations").iter()
				.map(|v|v.as_f64().expect("bad value in destinations") as usize).collect()),
			"extra_random_destinations" => extra_random_destinations=Some(
				value.as_f64().unwrap_or_else(|_|panic!("bad value for extra_random_destinations ({:?})",value)) as usize),
		);
		let destinations=destinations.unwrap_or_default();
		let extra_random_destinations=extra_random_destinations.unwrap_or(0);
		Hotspots{
			destinations,
			extra_random_destinations,
		}
	}
}

/// Use either of several patterns, with probability proportional to a weight.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomMix
{
	///The patterns in the pool to be selected.
	patterns: Vec<Box<dyn Pattern>>,
	///The given weights, one per pattern.
	weights: Vec<usize>,
	///A total weight computed at initialization.
	total_weight: usize,
}

impl Pattern for RandomMix
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		if self.patterns.len()!=self.weights.len()
		{
			panic!("Number of patterns must match number of weights for the RandomMix meta-pattern.");
		}
		if self.patterns.is_empty()
		{
			panic!("RandomMix requires at least one pattern (and 2 to be sensible).");
		}
		for pat in self.patterns.iter_mut()
		{
			pat.initialize(source_size,target_size,topology,rng);
		}
		self.total_weight=self.weights.iter().sum();
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut w = rng.gen_range(0..self.total_weight);
		let mut index = 0;
		while w>self.weights[index]
		{
			w-=self.weights[index];
			index+=1;
		}
		self.patterns[index].get_destination(origin,topology,rng)
	}
}

impl RandomMix
{
	fn new(arg:PatternBuilderArgument) -> RandomMix
	{
		let mut patterns=None;
		let mut weights=None;
		match_object_panic!(arg.cv,"RandomMix",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"weights" => weights=Some(value.as_array().expect("bad value for weights").iter()
				.map(|v|v.as_f64().expect("bad value in weights") as usize).collect()),
		);
		let patterns=patterns.expect("There were no patterns");
		let weights=weights.expect("There were no weights");
		RandomMix{
			patterns,
			weights,
			total_weight:0,//to be computed later
		}
	}
}

///It keeps a shuffled list, global for all sources, of destinations to which send. Once all have sent it is rebuilt and shuffled again.
///Independently of past requests, decisions or origin.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct GloballyShufflingDestinations
{
	///Number of destinations.
	size: usize,
	///Pending destinations.
	pending: RefCell<Vec<usize>>,
}

impl Pattern for GloballyShufflingDestinations
{
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.size=target_size;
		self.pending=RefCell::new(Vec::with_capacity(self.size));
		//if source_size!=target_size
		//{
		//	unimplemented!("Different sizes are not yet implemented for GloballyShufflingDestinations");
		//}
	}
	fn get_destination(&self, _origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut pending = self.pending.borrow_mut();
		if pending.is_empty()
		{
			for i in 0..self.size
			{
				pending.push(i);
			}
			//rng.shuffle(&mut pending);//rand-0.4
			pending.shuffle(rng);//rand-0.8
		}
		pending.pop().unwrap()
	}
}

impl GloballyShufflingDestinations
{
	fn new(arg:PatternBuilderArgument) -> GloballyShufflingDestinations
	{
		match_object_panic!(arg.cv,"GloballyShufflingDestinations",_value);
		GloballyShufflingDestinations{
			size:0,//to be filled in initialization
			pending:RefCell::new(Vec::new()),//to be filled in initialization
		}
	}
}

///For each group, it keeps a shuffled list of destinations to which send. Once all have sent it is rebuilt and shuffled again.
///Independently of past requests, decisions or origin.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct GroupShufflingDestinations
{
	///The size of each group.
	group_size: usize,
	///Number of destinations, in total.
	size: usize,
	///Pending destinations.
	pending: Vec<RefCell<Vec<usize>>>,
}

impl Pattern for GroupShufflingDestinations
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.size = target_size;
		let number_of_groups = (source_size+self.group_size-1) / self.group_size;// ts/gs rounded up
		self.pending=vec![RefCell::new(Vec::with_capacity(self.size)) ; number_of_groups];
		//if source_size!=target_size
		//{
		//	unimplemented!("Different sizes are not yet implemented for GroupShufflingDestinations");
		//}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let group = origin / self.group_size;
		let mut pending = self.pending[group].borrow_mut();
		if pending.is_empty()
		{
			for i in 0..self.size
			{
				pending.push(i);
			}
			//rng.shuffle(&mut pending);//rand-0.4
			pending.shuffle(rng);//rand-0.8
		}
		pending.pop().unwrap()
	}
}

impl GroupShufflingDestinations
{
	fn new(arg:PatternBuilderArgument) -> GroupShufflingDestinations
	{
		let mut group_size = None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="GroupShufflingDestinations"
			{
				panic!("A GroupShufflingDestinations must be created from a `GroupShufflingDestinations` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"group_size" => match value
					{
						&ConfigurationValue::Number(f) => group_size=Some(f as usize),
						_ => panic!("bad value for group_size"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in GroupShufflingDestinations",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a GroupShufflingDestinations from a non-Object");
		}
		let group_size = group_size.expect("There was no group_size");
		GroupShufflingDestinations{
			group_size,
			size:0,//to be filled in initialization
			pending:vec![],//to be filled in initialization
		}
	}
}


/**
Each message gets its destination sampled uniformly at random among the servers attached to neighbour routers.
It may build a pattern either of servers or switches, controlled through the `switch_level` configuration flag.
This pattern autoscales if requested a size multiple of the network size.

Example configuration:
```ignore
UniformDistance{
	///The distance at which the destination must be from the source.
	distance: 1,
	/// Optionally build the pattern at the switches. This should be irrelevant at direct network with the same number of servers per switch.
	//switch_level: true,
	legend_name: "uniform among neighbours",
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformDistance
{
	///Distance to which destinations must chosen.
	distance: usize,
	///Whether the pattern is defined at the switches, or otherwise, at the servers.
	switch_level: bool,
	///sources/destinations mapped to each router/server (depending on `switch_level`).
	concentration: usize,
	///`pool[i]` contains the routers at `distance` from the router `i`. 
	pool: Vec<Vec<usize>>,
}

impl Pattern for UniformDistance
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, _rng: &mut StdRng)
	{
		let n= if self.switch_level { topology.num_routers() } else { topology.num_servers() };
		//assert!(n==source_size && n==target_size,"The UniformDistance pattern needs source_size({})==target_size({})==num_routers({})",source_size,target_size,n);
		assert!(source_size==target_size,"The UniformDistance pattern needs source_size({})==target_size({})",source_size,target_size);
		assert!(source_size%n == 0,"The UniformDistance pattern needs the number of {}({}) to be a divisor of source_size({})",if self.switch_level { "routers" } else { "servers" },n,source_size);
		self.concentration = source_size/n;
		self.pool.reserve(n);
		for i in 0..n
		{
			let source = if self.switch_level { i } else {
				match topology.server_neighbour(i).0 {
					Location::RouterPort{
						router_index,
						router_port:_,
					} => router_index,
					_ => panic!("unconnected server"),
				}
			};
			let mut found: Vec<usize> = (0..n).filter(|&j|{
				let destination = if self.switch_level { j } else {
					match topology.server_neighbour(j).0 {
						Location::RouterPort{
							router_index,
							router_port:_,
						} => router_index,
						_ => panic!("unconnected server"),
					}
				};
				topology.distance(source,destination)==self.distance
			}).collect();
			found.shrink_to_fit();
			self.pool.push(found);
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let pool = &self.pool[origin/self.concentration];
		let r=rng.gen_range(0..pool.len());
		pool[r]*self.concentration + (origin%self.concentration)
	}
}

impl UniformDistance
{
	fn new(arg:PatternBuilderArgument) -> UniformDistance
	{
		let mut distance =  None;
		let mut switch_level =  false;
		match_object_panic!(arg.cv,"UniformDistance",value,
			"distance" => distance=Some(value.as_f64().expect("bad value for distance") as usize),
			"switch_level" => switch_level = value.as_bool().expect("bad value for switch_level"),
		);
		let distance = distance.expect("There were no distance");
		UniformDistance{
			distance,
			switch_level,
			concentration:0,//to be filled on initialization
			pool: vec![],//to be filled oninitialization
		}
	}
}

///Build a random map on initialization, which is then kept constant.
///Optionally allow self-messages.
///See `RandomPermutation` and `FileMap`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FixedRandom
{
	map: Vec<usize>,
	allow_self: bool,
	opt_rng: Option<StdRng>,
}

impl Pattern for FixedRandom
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
	{
		self.map.reserve(source_size);
		let rng= self.opt_rng.as_mut().unwrap_or(rng);
		for source in 0..source_size
		{
			// To avoid selecting self we substract 1 from the total. If the random falls in the latter half we add it again.
			let n = if self.allow_self || target_size<source { target_size } else { target_size -1 };
			let mut elem = rng.gen_range(0..n);
			if !self.allow_self && elem>=source
			{
				elem += 1;
			}
			self.map.push(elem);
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		self.map[origin]
	}
}

impl FixedRandom
{
	fn new(arg:PatternBuilderArgument) -> FixedRandom
	{
		let mut allow_self = false;
		let mut opt_rng = None; 
		match_object_panic!(arg.cv,"FixedRandom",value,
			"seed" => opt_rng = Some( StdRng::seed_from_u64(
				value.as_f64().expect("bad value for seed") as u64
			)),
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
		FixedRandom{
			map: vec![],//to be intializated
			allow_self,
			opt_rng,
		}
	}
}


/// Partition the nodes in independent regions, each with its own pattern. Source and target sizes must be equal.
/// ```ignore
/// IndependentRegions{
/// 	// An array with the patterns for each region.
/// 	patterns: [Uniform, Hotspots{destinations:[0]}],
/// 	// An array with the size of each region. They must add up to the total size.
/// 	sizes: [100, 50],
/// 	// Alternatively, use relative_sizes. the pattern will be initialized with sizes proportional to these.
/// 	// You must use exactly one of either `sizes` or `relative_sizes`.
/// 	// relative_sizes: [88, 11],
/// }
/// ```
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct IndependentRegions
{
	/// The actual size of each region. An empty vector if not given nor initialized.
	/// If not empty it must sum up to the total size and have as many elements as the `patterns` field.
	sizes: Vec<usize>,
	/// The pattern to be employed in each region.
	patterns: Vec<Box<dyn Pattern>>,
	/// If not empty, it is used to build the actual `sizes`.
	relative_sizes: Vec<f64>,
}

/**
Build an integer vector with elements proportional to the given `weights` and with a total `target_sum`.
Based on <https://stackoverflow.com/questions/16226991/allocate-an-array-of-integers-proportionally-compensating-for-rounding-errors>
**/
pub fn proportional_vec_with_sum(weights:&Vec<f64>, target_sum:usize) -> Vec<usize>
{
	let mut result : Vec<usize> = Vec::with_capacity(weights.len());
	let mut total_weight : f64 = weights.iter().sum();
	let mut target_sum : f64 = target_sum as f64;
	for &w in weights
	{
		let rounded : f64 = ( w*target_sum/total_weight ).round();
		result.push(rounded as usize);
		total_weight -= w;
		target_sum -= rounded;
	}
	result
}

impl Pattern for IndependentRegions
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		assert!(source_size==target_size, "source_size and target_size must be equal in IndependentRegions.");
		if !self.relative_sizes.is_empty()
		{
			assert!(self.sizes.is_empty(),"Cannot set both sizes and relative_sizes in IndependentRegions.");
			// Just doing this do not work. Consider [37,37,74] for 150, which gives [38,38,75].
			//let relative_total: f64 = self.relative_sizes.iter().sum();
			//let scale : f64 = source_size as f64 / relative_total;
			//let expected_sizes : Vec<f64> = self.relative_sizes.iter().map(|x|x*scale).collect();
			//self.sizes = expected_sizes.iter().map(|x|x.round() as usize).collect();
			//TODO: Is this guaranteed to sum correctly??
			self.sizes = proportional_vec_with_sum(&self.relative_sizes,source_size);
		}
		assert!(self.sizes.iter().sum::<usize>()==source_size,"IndependentRegions sizes {:?} do not add up to the source_size {}",self.sizes,source_size);
		for region_index in 0..self.patterns.len()
		{
			let size = self.sizes[region_index];
			self.patterns[region_index].initialize(size,size,topology,rng);
		}
	}
	fn get_destination(&self, mut origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut region_index = 0;
		let mut region_offset = 0;
		while origin >= self.sizes[region_index]
		{
			origin -= self.sizes[region_index];
			region_offset += self.sizes[region_index];
			region_index += 1;
		}
		let destination = self.patterns[region_index].get_destination(origin,topology,rng);
		destination + region_offset
	}
}

impl IndependentRegions
{
	fn new(arg:PatternBuilderArgument) -> IndependentRegions
	{
		let mut patterns : Option<Vec<_>> = None;
		let mut sizes = None;
		let mut relative_sizes = None;
		match_object_panic!(arg.cv,"IndependentRegions",value,
			"patterns" => patterns = Some(value.as_array().expect("bad value for patterns").iter()
				.map(|v|new_pattern(PatternBuilderArgument{cv:v,..arg})).collect()),
			"sizes" => sizes = Some(value.as_array()
				.expect("bad value for sizes").iter()
				.map(|v|v.as_f64().expect("bad value in sizes") as usize).collect()),
			"relative_sizes" => relative_sizes = Some(value.as_array()
				.expect("bad value for relative_sizes").iter()
				.map(|v|v.as_f64().expect("bad value in relative_sizes")).collect()),
		);
		let patterns = patterns.expect("There was no patterns.");
		assert!( matches!(sizes,None) || matches!(relative_sizes,None), "Cannot set both sizes and relative_sizes." );
		assert!( !matches!(sizes,None) || !matches!(relative_sizes,None), "Must set one of sizes or relative_sizes." );
		let sizes = sizes.unwrap_or_else(||Vec::new());
		let relative_sizes = relative_sizes.unwrap_or_else(||Vec::new());
		assert!(patterns.len()==sizes.len().max(relative_sizes.len()),"Different number of entries in IndependentRegions.");
		IndependentRegions{
			patterns,
			sizes,
			relative_sizes,
		}
	}
}



/**
A pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying
some criteria. Note this is only a pattern, the actual packet route does not have to go throught such middle router.
It has the same implicit concentration scaling as UniformDistance, allowing building a pattern over a multiple of the number of switches.

Example configuration:
```ignore
RestrictedMiddleUniform{
	/// An optional integer value to allow only middle routers whose index is greater or equal to it.
	minimum_index: 100,
	/// An optional integer value to allow only middle routers whose index is lower or equal to it.
	// maximum_index: 100,
	/// Optionally, give a vector with the possible values of the distance from the source to the middle.
	distances_to_source: [1],
	/// Optionally, give a vector with the possible values of the distance from the middle to the destination.
	distances_to_destination: [1],
	/// Optionally, a vector with distances from source to destination, ignoring middle.
	distances_source_to_destination: [2],
	/// Optionally, set a pattern for those sources with no legal destination.
	else: Uniform,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RestrictedMiddleUniform
{
	minimum_index: Option<usize>,
	maximum_index: Option<usize>,
	distances_to_source: Option<Vec<usize>>,
	distances_to_destination: Option<Vec<usize>>,
	distances_source_to_destination: Option<Vec<usize>>,
	else_pattern: Option<Box<dyn Pattern>>,
	///Whether the pattern is defined at the switches, or otherwise, at the servers.
	switch_level: bool,
	/// sources/destinations mapped to each router. An implicit product to ease the normal case.
	concentration: usize,
	///`pool[i]` contains the routers at `distance` from the router `i`. 
	pool: Vec<Vec<usize>>,
}

impl Pattern for RestrictedMiddleUniform
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		let n= if self.switch_level { topology.num_routers() } else { topology.num_servers() };
		//assert!(n==source_size && n==target_size,"The RestrictedMiddleUniform pattern needs source_size({})==target_size({})==num_routers({})",source_size,target_size,n);
		assert!(source_size==target_size,"The RestrictedMiddleUniform pattern needs source_size({})==target_size({})",source_size,target_size);
		assert!(source_size%n == 0,"The RestrictedMiddleUniform pattern needs the number of {}({}) to be a divisor of source_size({})",if self.switch_level { "routers" } else { "servers" },n,source_size);
		self.concentration = source_size/n;
		self.pool.reserve(n);
		let middle_min = self.minimum_index.unwrap_or(0);
		let middle_max = self.maximum_index.unwrap_or_else(||topology.num_routers()-1);
		for source in 0..n
		{
			let source_switch = if self.switch_level { source } else {
				match topology.server_neighbour(source).0 {
					Location::RouterPort{
						router_index,
						router_port:_,
					} => router_index,
					_ => panic!("unconnected server"),
				}
			};
			// --- There are two main ways to proceed:
			// --- to run over the n^2 pairs of source/destination, filtering out by middle.
			// --- to run first over possible middle switches and then over destinations. But with this destinations appear for several middles and have to be cleaned up. This way could be more efficient for small distances if employing the neighbour function.
			//let mut found: Vec<usize> = (middle_min..=middle_max).flat_map(|&middle|{
			//	// First check criteria between source and middle
			//	if let Some(ref dists) = self.distances_to_source
			//	{
			//		let d = topology.distance(source,middle);
			//		if !dists.contains(&d) { return vec![]; }
			//	}
			//	// Now look for the destinations satisfying all the criteria.
			//	(0..n).filter(|destination|{
			//		let mut good = true;
			//		if let Some(ref dists) = self.distances_to_destination
			//		{
			//			let d = topology.distance(middle,destination);
			//			if !dists.contains(&d) { good=false; }
			//		}
			//		// we would add other criteria checks here.
			//		good
			//	}).collect()
			//}).collect();
			let mut found: Vec<usize> = (0..n).filter(|&destination|{
				let destination_switch = if self.switch_level { destination } else {
					match topology.server_neighbour(destination).0 {
						Location::RouterPort{
							router_index,
							router_port:_,
						} => router_index,
						_ => panic!("unconnected server"),
					}
				};
				for middle in middle_min..=middle_max
				{
					if let Some(ref dists) = self.distances_to_source
					{
						let d = topology.distance(source_switch,middle);
						if !dists.contains(&d) { continue; }
					}
					if let Some(ref dists) = self.distances_to_destination
					{
						let d = topology.distance(middle,destination_switch);
						if !dists.contains(&d) { continue; }
					}
					if let Some(ref dists) = self.distances_source_to_destination
					{
						let d = topology.distance(source_switch,destination_switch);
						if !dists.contains(&d) { continue; }
					}
					return true;
				}
				false
			}).collect();
			if self.else_pattern.is_none(){
				assert!(!found.is_empty(),"RestrictedMiddleUniform: Empty set of destinations for switch {} and there is no else clause set.",source_switch);
			}
			found.shrink_to_fit();
			self.pool.push(found);
		}
		if let Some(ref mut pat) = self.else_pattern
		{
			pat.initialize(source_size,target_size,topology,rng);
		}
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let pool = &self.pool[origin/self.concentration];
		if pool.is_empty() {
			self.else_pattern.as_ref().expect("else clause should be set").get_destination(origin,topology,rng)
		} else {
			let r=rng.gen_range(0..pool.len());
			pool[r]*self.concentration + (origin%self.concentration)
		}
	}
}


/**
The node at an `index` sends traffic randomly to one of `index+g`, where `g` is any of the declared `generators`.
These sums are made modulo the destination size, which is intended to be equal the source size.
the induced communication matrix is a Circulant matrix, hence its name.

In this example each node `x` send to either `x+1` or `x+2`.
```ignore
Circulant{
	generators: [1,2],
}
```
**/
#[derive(Quantifiable,Debug)]
pub struct Circulant
{
	//config:
	///The generators to be employed.
	pub generators: Vec<i32>,
	//intialized:
	///The size of the destinations set, captured at initialization.
	pub size: i32,
}

impl Pattern for Circulant
{
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.size = target_size as i32;
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let r = rng.gen_range(0..self.generators.len());
		let gen = self.generators[r];
		// Note the '%' operator keeps the argument sign, so we use rem_euclid.
		(origin as i32+gen).rem_euclid(self.size) as usize
	}
}

impl Circulant
{
	fn new(arg:PatternBuilderArgument) -> Circulant
	{
		let mut generators = vec![];
		match_object_panic!(arg.cv,"Circulant",value,
			"generators" => generators=value.as_array().expect("bad value for generators").iter()
				.map(|v|v.as_i32().expect("bad value in generators")).collect(),
		);
		if generators.is_empty()
		{
			panic!("cannot build a Circulant pattern with empty set of generators.");
		}
		Circulant{
			generators,
			size:0,
		}
	}
}


impl RestrictedMiddleUniform
{
	fn new(arg:PatternBuilderArgument) -> RestrictedMiddleUniform
	{
		let mut minimum_index = None;
		let mut maximum_index = None;
		let mut distances_to_source = None;
		let mut distances_to_destination = None;
		let mut distances_source_to_destination = None;
		let mut else_pattern = None;
		let mut switch_level =  false;
		match_object_panic!(arg.cv,"RestrictedMiddleUniform",value,
			"minimum_index" => minimum_index=Some(value.as_f64().expect("bad value for minimum_index") as usize),
			"maximum_index" => maximum_index=Some(value.as_f64().expect("bad value for maximum_index") as usize),
			"distances_to_source" => distances_to_source=Some(
				value.as_array().expect("bad value for distances_to_source").iter().map(
				|x|x.as_f64().expect("bad value for distances_to_source") as usize
			).collect()),
			"distances_to_destination" => distances_to_destination=Some(
				value.as_array().expect("bad value for distances_to_destination").iter().map(
				|x|x.as_f64().expect("bad value for distances_to_destination") as usize
			).collect()),
			"distances_source_to_destination" => distances_source_to_destination=Some(
				value.as_array().expect("bad value for distances_source_to_destination").iter().map(
				|x|x.as_f64().expect("bad value for distances_source_to_destination") as usize
			).collect()),
			"else" => else_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"switch_level" => switch_level = value.as_bool().expect("bad value for switch_level"),
		);
		RestrictedMiddleUniform{
			minimum_index,
			maximum_index,
			distances_to_source,
			distances_to_destination,
			distances_source_to_destination,
			else_pattern,
			switch_level,
			concentration:0,//to be filled on initialization
			pool: vec![],//to be filled oninitialization
		}
	}
}

/**
Maps from a block into another following the natural embedding, keeping the corrdinates of every node.
Both block must have the same number of dimensions, and each dimension should be greater at the destination than at the source.
This is intended to be used to place several small applications in a larger machine.
It can combined with [CartesianTransform] to be placed at an offset, to set a stride, or others.

Example mapping nodes in a block of 16 nodes into one of 64 nodes.
```ignore
CartesianEmbedding{
	source_sides: [4,4],
	destination_sides: [8,8],
}
```
**/
#[derive(Debug,Quantifiable)]
pub struct CartesianEmbedding
{
	source_cartesian_data: CartesianData,
	destination_cartesian_data: CartesianData,
}

impl Pattern for CartesianEmbedding
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!=self.source_cartesian_data.size
		{
			panic!("Source sizes do not agree on CartesianEmbedding.");
		}
		if target_size!=self.destination_cartesian_data.size
		{
			panic!("Detination sizes do not agree on CartesianEmbedding.");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		let up_origin=self.source_cartesian_data.unpack(origin);
		self.destination_cartesian_data.pack(&up_origin)
	}
}

impl CartesianEmbedding
{
	pub fn new(arg:PatternBuilderArgument) -> CartesianEmbedding
	{
		let mut source_sides:Option<Vec<_>>=None;
		let mut destination_sides:Option<Vec<_>>=None;
		match_object_panic!(arg.cv,"CartesianEmbedding",value,
			"source_sides" => source_sides = Some(value.as_array().expect("bad value for source_sides").iter()
				.map(|v|v.as_usize().expect("bad value in source_sides")).collect()),
			"destination_sides" => destination_sides = Some(value.as_array().expect("bad value for destination_sides").iter()
				.map(|v|v.as_usize().expect("bad value in destination_sides")).collect()),
		);
		let source_sides=source_sides.expect("There were no source_sides");
		let destination_sides=destination_sides.expect("There were no destination_sides");
		if source_sides.len() != destination_sides.len()
		{
			panic!("Different number of dimensions in CartesianEmbedding.")
		}
		for (index,(ss, ds)) in std::iter::zip( source_sides.iter(), destination_sides.iter() ).enumerate()
		{
			if ss>ds
			{
				panic!("Source is greater than destination at side {index}. {ss}>{ds}",index=index,ss=ss,ds=ds);
			}
		}
		CartesianEmbedding{
			source_cartesian_data: CartesianData::new(&source_sides),
			destination_cartesian_data: CartesianData::new(&destination_sides),
		}
	}
}

/**
Apply some other [Pattern] over a set of nodes whose indices have been remapped according to a [Pattern]-given permutation.
A source `x` chooses as destination `map(pattern(invmap(x)))`, where `map` is the given permutation, `invmap` its inverse and `pattern` is the underlaying pattern to apply. In other words, if `pattern(a)=b`, then destination of `map(a)` is set to `map(b)`. It can be seen as a [Composition] that manages building the inverse map.

Remapped nodes requires source and destination to be of the same size. The pattern creating the map is called once and must return in a permutation, as to be able to make its inverse.

For a similar operation on other types see [RemappedServersTopology].

Example building a cycle in random order.
```ignore
RemappedNodes{
	/// The underlaying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

**/
#[derive(Debug,Quantifiable)]
struct RemappedNodes
{
	/// Maps from inner indices to outer indices.
	/// It must be a permutation.
	from_base_map: Vec<usize>,
	/// Maps from outer indices to inner indices.
	/// The inverse of `from_base_map`.
	into_base_map: Vec<usize>,
	/// The inner pattern to be applied.
	pattern: Box<dyn Pattern>,
	/// The pattern to build the map vectors.
	map: Box<dyn Pattern>,
}

impl Pattern for RemappedNodes
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		if source_size != target_size
		{
			panic!("RemappedNodes requires source and target sets to have same size.");
		}
		let n = source_size;
		self.map.initialize(n,n,topology,rng);
		self.from_base_map = (0..n).map(|inner_index|{
			self.map.get_destination(inner_index,topology,rng)
		}).collect();
		let mut into_base_map = vec![None;n];
		for (inside,&outside) in self.from_base_map.iter().enumerate()
		{
			match into_base_map[outside]
			{
				None => into_base_map[outside]=Some(inside),
				Some(already_inside) => panic!("Two inside nodes ({inside} and {already_inside}) mapped to the same outer index ({outside}).",inside=inside,already_inside=already_inside,outside=outside),
			}
		}
		self.into_base_map = into_base_map.iter().map(|x|x.expect("node not mapped")).collect();
		self.pattern.initialize(n,n,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let inner_origin = self.into_base_map[origin];
		let inner_dest = self.pattern.get_destination(inner_origin,topology,rng);
		self.from_base_map[inner_dest]
	}
}

impl RemappedNodes
{
	fn new(arg:PatternBuilderArgument) -> RemappedNodes
	{
		let mut pattern = None;
		let mut map = None;
		match_object_panic!(arg.cv, "RemappedNodes", value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"map" => map = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
		);
		let pattern = pattern.expect("There were no pattern in configuration of RemappedServersTopology.");
		let map = map.expect("There were no map in configuration of RemappedServersTopology.");
		RemappedNodes{
			from_base_map: vec![],
			into_base_map: vec![],
			pattern,
			map,
		}
	}
}



#[cfg(test)]
mod tests {
	use super::*;
	use rand::SeedableRng;
	#[test]
	fn uniform_test()
	{
		let plugs = Plugs::default();
		let mut rng=StdRng::seed_from_u64(10u64);
		use crate::topology::{new_topology,TopologyBuilderArgument};
		// TODO: topology::dummy?
		let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);
		let dummy_topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});
		for origin_size in [10,20]
		{
			for destination_size in [10,20]
			{
				for allow_self in [true,false]
				{
					let cv_allow_self = if allow_self { ConfigurationValue::True } else { ConfigurationValue::False };
					let cv = ConfigurationValue::Object("Uniform".to_string(),vec![("allow_self".to_string(),cv_allow_self)]);
					let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
					let mut uniform = UniformPattern::new(arg);
					uniform.initialize(origin_size,destination_size,&*dummy_topology,&mut rng);
					let sample_size = (origin_size+destination_size)*10;
					let origin=5;
					let mut counts = vec![0;destination_size];
					for _ in 0..sample_size
					{
						let destination = uniform.get_destination(origin,&*dummy_topology,&mut rng);
						assert!(destination<destination_size, "bad destination from {} into {} (allow_self:{}) got {}",origin_size,destination_size,allow_self,destination);
						counts[destination]+=1;
					}
					assert!( (allow_self && counts[origin]>0) || (!allow_self && counts[origin]==0) , "allow_self failing");
					for (dest,&count) in counts.iter().enumerate()
					{
						assert!( dest==origin || count>0, "missing elements at index {} from {} into {} (allow_self:{})",dest,origin_size,destination_size,allow_self);
					}
				}
			}
		}
	}
	#[test]
	fn fixed_random_self()
	{
		let plugs = Plugs::default();
		let cv = ConfigurationValue::Object("FixedRandom".to_string(),vec![("allow_self".to_string(),ConfigurationValue::True)]);
		let mut rng=StdRng::seed_from_u64(10u64);
		use crate::topology::{new_topology,TopologyBuilderArgument};
		// TODO: topology::dummy?
		let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);
		let dummy_topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});
		
		for size in [1000]
		{
			let mut count = 0;
			let sizef = size as f64;
			let sample_size = 100;
			let expected_unique = sizef* ( (sizef-1.0)/sizef ).powf(sizef-1.0) * sample_size as f64;
			let mut unique_count = 0;
			for _ in 0..sample_size
			{
				let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
				let mut with_self = FixedRandom::new(arg);
				with_self.initialize(size,size,&*dummy_topology,&mut rng);
				let mut dests = vec![0;size];
				for origin in 0..size
				{
					let destination = with_self.get_destination(origin,&*dummy_topology,&mut rng);
					if destination==origin
					{
						count+=1;
					}
					dests[destination]+=1;
				}
				unique_count += dests.iter().filter(|&&x|x==1).count();
			}
			assert!( count>=sample_size-1,"too few self messages {}, expecting {}",count,sample_size);
			assert!( count<=sample_size+1,"too many self messages {}, expecting {}",count,sample_size);
			assert!( (unique_count as f64) >= expected_unique*0.99 ,"too few unique destinations {}, expecting {}",unique_count,expected_unique);
			assert!( (unique_count as f64) <= expected_unique*1.01 ,"too many unique destinations {}, expecting {}",unique_count,expected_unique);
		}
		
		let cv = ConfigurationValue::Object("FixedRandom".to_string(),vec![("allow_self".to_string(),ConfigurationValue::False)]);
		for logsize in 1..10
		{
			let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
			let size = 2usize.pow(logsize);
			let mut without_self = FixedRandom::new(arg);
			without_self.initialize(size,size,&*dummy_topology,&mut rng);
			let count = (0..size).filter( |&origin| origin==without_self.get_destination(origin,&*dummy_topology,&mut rng) ).count();
			assert!(count==0, "Got {} selfs at size {}.", count, size );
		}
	}
}

