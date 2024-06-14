/*!

A [Pattern] defines the way elements select their destinations.

see [`new_pattern`](fn.new_pattern.html) for documentation on the configuration syntax of predefined patterns.

*/

use std::cell::{RefCell};
use std::collections::VecDeque;
use std::convert::TryInto;
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom};
use std::fs::File;
use std::io::{BufRead,BufReader};

use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology, Location};
use crate::quantify::Quantifiable;
use crate::{Plugs,match_object_panic};
use rand::{RngCore, SeedableRng};

/// Some things most uses of the pattern module will use.
pub mod prelude
{
	pub use super::{Pattern,new_pattern,PatternBuilderArgument};
}

///A `Pattern` describes how a set of entities decides destinations into another set of entities.
///The entities are initially servers, but after some operators it may mean router, rows/columns, or other groupings.
///The source and target set may be or not be the same. Or even be of different size.
///Thus, a `Pattern` is a generalization of the mathematical concept of function.
pub trait Pattern : Quantifiable + std::fmt::Debug
{
	//Indices are either servers or virtual things.
	///Fix the input and output size, providing the topology and random number generator.
	///Careful with using topology in sub-patterns. For example, it may be misleading to use the dragonfly topology when
	///building a pattern among groups or a pattern among the routers of a single group.
	///Even just a pattern of routers instead of a pattern of servers can lead to mistakes.
	///Read the documentation of the traffic or meta-pattern using the pattern to know what its their input and output.
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng);
	///Obtain a destination of a source. This will be called repeatedly as the traffic requires destination for its messages.
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize;
}

///The argument to a builder function of patterns.
#[derive(Debug)]
pub struct PatternBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the pattern.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the pattern needs to create elements.
	pub plugs: &'a Plugs,
}

impl<'a> PatternBuilderArgument<'a>
{
	fn with_cv<'b>(&'b self, new_cv:&'b ConfigurationValue) -> PatternBuilderArgument<'b>
	{
		PatternBuilderArgument{
			cv: new_cv,
			plugs: self.plugs,
		}
	}
}


/**Build a new pattern. Patterns are maps between two sets which may depend on the RNG. Generally over the whole set of servers, but sometimes among routers or groups. Check the documentation of the parent Traffic/Permutation for its interpretation.

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
[RestrictedMiddleUniform] is a pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying some criteria. Note this is only a pattern, the actual packet route does not have to go through such middle router.
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
With [FileMap] a map is read from a file. Each element has a unique destination.
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```

### CartesianTransform
With [CartesianTransform] the nodes are seen as in a n-dimensional orthohedro. Then it applies several transformations. When mapping directly servers it may be useful to use as `sides[0]` the number of servers per router.
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
	/// The underlying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

### CartesianCut

With [CartesianCut] you see the nodes as block with an embedded block. Then you define a pattern inside the small block and another outside. See [CartesianCut] for details and examples.
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
			"LinearTransform" => Box::new(LinearTransform::new(arg)),
			"CartesianTiling" => Box::new(CartesianTiling::new(arg)),
			"Composition" => Box::new(Composition::new(arg)),
			"Pow" => Box::new(Pow::new(arg)),
			"CartesianFactor" => Box::new(CartesianFactor::new(arg)),
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
			"CartesianCut" => Box::new(CartesianCut::new(arg)),
			"RemappedNodes" => Box::new(RemappedNodes::new(arg)),
			"Switch" => Box::new(Switch::new(arg)),
			"Debug" => Box::new(DebugPattern::new(arg)),
			"MiDebugPattern" => Box::new(MiDebugPattern::new(arg)),
			"DestinationSets" => Box::new(DestinationSets::new(arg)),
			"ElementComposition" => Box::new(ElementComposition::new(arg)),
			"CandidatesSelection" => Box::new(CandidatesSelection::new(arg)),
			"Sum" => Box::new(Sum::new(arg)),
			"RoundRobin" => Box::new(RoundRobin::new(arg)),
			"Inverse" => Box::new(Inverse::new(arg)),
			"SubApp" => Box::new(SubApp::new(arg)),
			"RecursiveDistanceHalving" => Box::new(RecursiveDistanceHalving::new(arg)),
			"BinomialTree" => Box::new(BinomialTree::new(arg)),
			"InmediateSequencePattern" => Box::new(InmediateSequencePattern::new(arg)),
			"Stencil" => EncapsulatedPattern::new(cv_name.clone(), arg),
			_ => panic!("Unknown pattern {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a Pattern from a non-Object");
	}
}

/// In case you want to build a list of patterns but some of them are optional.
pub fn new_optional_pattern(arg:PatternBuilderArgument) -> Option<Box<dyn Pattern>>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		match cv_name.as_ref()
		{
			"None" => None,
			_ => Some(new_pattern(arg))
		}
	}else {
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

/**
Build a random permutation on initialization, which is then kept constant.
This allows self-messages; with a reasonable probability of having one.
Has `random_seed` as optional configuration to use an internal random-number generator instead of the simulation-wide one.

See [RandomInvolution] and [FileMap].
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomPermutation
{
	permutation: Vec<usize>,
	rng: Option<StdRng>,
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
		let rng= self.rng.as_mut().unwrap_or(rng);
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
		let mut rng = None;
		match_object_panic!(arg.cv,"RandomPermutation",value,
			"seed" => rng = Some( value.as_rng().expect("bad value for seed") ),
		);
		RandomPermutation{
			permutation: vec![],
			rng,
		}
	}
}

///Build a random involution on initialization, which is then kept constant.
///An involution is a permutation that is a pairing/matching; if `a` is the destination of `b` then `b` is the destination of `a`.
///It will panic if given an odd size.
///See [RandomPermutation].
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
		assert_eq!(source_size % 2, 0);
		//Todo: annotate this weird algorithm.
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
///Then apply the `global_pattern` among the components and select randomly inside the destination component.
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
			panic!("In a CartesianTransform source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		if source_size!=self.cartesian_data.size
		{
			panic!("In a CartesianTransform source_size({}) must be equal to cartesian size({}).",source_size,self.cartesian_data.size);
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
		assert_eq!(source_size % factor, 0);
		assert_eq!(target_size % factor, 0);
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
		assert_eq!(n, repetitions.len());
		let final_sides : Vec<_> = (0..n).map(|index|sides[index]*repetitions[index]).collect();
		CartesianTiling{
			pattern,
			base_cartesian_data: CartesianData::new(&sides),
			repetitions,
			final_cartesian_data: CartesianData::new(&final_sides),
		}
	}
}


/**
The pattern resulting of composing a list of patterns.
`destination=patterns[len-1]( patterns[len-2] ( ... (patterns[1] ( patterns[0]( origin ) )) ) )`.
The intermediate sizes along the composition can be stated by `middle_sizes`, otherwise they are set equal to the `target_size` of the whole.
Thus in a composition of two patterns in which the midddle size is `x`and not equal to `target_size`, it should be set `middle_sizes=[x]`.
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Composition
{
	patterns: Vec<Box<dyn Pattern>>,
	middle_sizes: Vec<usize>,
}

impl Pattern for Composition
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		for (index,pattern) in self.patterns.iter_mut().enumerate()
		{
			let current_source = if index==0 { source_size } else { *self.middle_sizes.get(index-1).unwrap_or(&target_size) };
			let current_target = *self.middle_sizes.get(index).unwrap_or(&target_size);
			pattern.initialize(current_source,current_target,topology,rng);
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
		let mut middle_sizes=None;
		match_object_panic!(arg.cv,"Composition",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"middle_sizes" => middle_sizes = Some(value.as_array().expect("bad value for middle_sizes").iter()
				.map(|v|v.as_usize().expect("bad value for middle_sizes")).collect()),
		);
		let patterns=patterns.expect("There were no patterns");
		let middle_sizes = middle_sizes.unwrap_or_else(||vec![]);
		Composition{
			patterns,
			middle_sizes,
		}
	}
}


/**
 For a source, it sums the result of applying several patterns.
 For instance, the destination of a server a would be: dest(a) = p1(a) + p2(a) + p3(a).
 middle_sizes indicates the size of the intermediate patters.

Sum{ //A vector of 2's
	patterns:[
		CandidatesSelection{
				pattern: Identity,
				pattern_destination_size: 2048,
		},
		CandidatesSelection{
				pattern: Identity,
				pattern_destination_size: 2048,
		},
	],
	middle_sizes: [2,2],
},
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sum
{
	patterns: Vec<Box<dyn Pattern>>,
	middle_sizes: Vec<usize>,
	target_size: Option<usize>,
}

impl Pattern for Sum
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		for (index,pattern) in self.patterns.iter_mut().enumerate()
		{
			// let current_source = if index==0 { source_size } else { *self.middle_sizes.get(index-1).unwrap_or(&target_size) };
			let current_target = *self.middle_sizes.get(index).unwrap_or(&target_size);
			pattern.initialize(source_size,current_target,topology,rng);
		}
		self.target_size = Some(target_size);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let target_size = self.target_size.unwrap();
		let mut destination=0;
		for pattern in self.patterns.iter()
		{
			let next_destination = pattern.get_destination(origin,topology,rng);
			destination+=next_destination;
		}
		if destination>=target_size
		{
			panic!("Sum pattern overflowed the target size.")
		}
		destination
	}
}

impl Sum
{
	fn new(arg:PatternBuilderArgument) -> Sum
	{
		let mut patterns=None;
		let mut middle_sizes=None;
		match_object_panic!(arg.cv,"Sum",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"middle_sizes" => middle_sizes = Some(value.as_array().expect("bad value for middle_sizes").iter()
				.map(|v|v.as_usize().expect("bad value for middle_sizes")).collect()),
		);
		let patterns=patterns.expect("There were no patterns");
		let middle_sizes = middle_sizes.unwrap_or_else(||vec![]);
		Sum{
			patterns,
			middle_sizes,
			target_size: None,
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

/**
Use a list of patterns in a round robin fashion, for each source.

RoundRobin{ // Alternate between three random permutations
	patterns: [RandomPermutation, RandomPermutation, RandomPermutation],
}
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RoundRobin
{
	///The patterns in the pool to be selected.
	patterns: Vec<Box<dyn Pattern>>,
	/// Vec pattern origin
	index: RefCell<Vec<usize>>,
}

impl Pattern for RoundRobin
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		if self.patterns.is_empty()
		{
			panic!("RoundRobin requires at least one pattern (and 2 to be sensible).");
		}
		for pat in self.patterns.iter_mut()
		{
			pat.initialize(source_size,target_size,topology,rng);
		}
		self.index.replace(vec![0;source_size]);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let mut indexes = self.index.borrow_mut();
		let pattern_index = indexes[origin];
		indexes[origin] = (pattern_index+1) % self.patterns.len();
		self.patterns[pattern_index].get_destination(origin,topology,rng)
	}
}

impl RoundRobin
{
	fn new(arg:PatternBuilderArgument) -> RoundRobin
	{
		let mut patterns=None;
		match_object_panic!(arg.cv,"RoundRobin",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
		);
		let patterns=patterns.expect("There were no patterns");
		RoundRobin{
			patterns,
			index: RefCell::new(Vec::new()),
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
For each server, it keeps a shuffled list of destinations to which send.
Select each destination with a probability.

TODO: describe `weights` parameter.

```ignore
DestinationSets{
	patterns: [RandomPermutation, RandomPermutation, RandomPermutation], //2 random destinations
	weights: [1, 1, 2], //First 25% of chances, second 25% of chances, and third 50% of chances of being chosen
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct DestinationSets
{
	///Patterns to get the set of destinations
	patterns: Vec<Box<dyn Pattern>>,
	///Weights for each pattern
	weights: Vec<usize>,
	///Set of destinations.
	destination_set: Vec<Vec<usize>>,
}

impl Pattern for DestinationSets
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		for (index,pattern) in self.patterns.iter_mut().enumerate()
		{
			pattern.initialize(source_size,target_size,topology,rng);
			for source in 0..source_size
			{
				let destination = pattern.get_destination(source,topology,rng);
				self.destination_set[index].push(destination);
			}
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let total_weight=self.weights.iter().sum();
		let mut w = rng.gen_range(0..total_weight);
		let mut index = 0;
		while w>self.weights[index]
		{
			w-=self.weights[index];
			index+=1;
		}
		self.destination_set[index][origin]
	}
}

impl DestinationSets
{
	fn new(arg:PatternBuilderArgument) -> DestinationSets
	{
		let mut patterns=None;
		let mut weights: Option<Vec<usize>>=None;
		match_object_panic!(arg.cv,"DestinationSets",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"weights" => weights=Some(value.as_array().expect("bad value for weights").iter()
				.map(|v|v.as_f64().expect("bad value in weights") as usize).collect()),
		);
		let patterns:Vec<Box<dyn Pattern>>=patterns.expect("There were no patterns");
		let weights = if let Some(ref weights)=weights
		{
			assert_eq!(patterns.len(),weights.len(),"The number of patterns must match the number of weights");
			weights.clone()
		}else {
			vec![1usize; patterns.len()]
		};
		let size = patterns.len();

		DestinationSets{
			patterns,
			weights,
			destination_set:vec![vec![];size],//to be filled in initialization
		}
	}
}


/**
For each server, it keeps a shuffled list of destinations to which send.
Select each destination with a probability.

```ignore
InmediateSequencePattern{

}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct InmediateSequencePattern
{
	sequence: Vec<usize>,
	///Sequence for each input
	sequences_input: RefCell<Vec<VecDeque<usize>>>,
}

impl Pattern for InmediateSequencePattern
{
	fn initialize(&mut self, source_size:usize, _target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		self.sequences_input.replace(vec![VecDeque::from(self.sequence.clone()); source_size]);

	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		self.sequences_input.borrow_mut()[origin].pop_front().unwrap_or(0)
	}
}

impl InmediateSequencePattern
{
	fn new(arg:PatternBuilderArgument) -> InmediateSequencePattern
	{
		let mut sequence=None;
		match_object_panic!(arg.cv,"InmediateSequencePattern",value,
			"sequence" => sequence=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|v|v.as_usize().expect("List should be of usizes")).collect()),
		);
		let sequence = sequence.unwrap();
		InmediateSequencePattern {
			sequence,
			sequences_input: RefCell::new(vec![VecDeque::new()]),
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
		assert_eq!(source_size, target_size, "The UniformDistance pattern needs source_size({})==target_size({})", source_size, target_size);
		assert_eq!(source_size % n, 0, "The UniformDistance pattern needs the number of {}({}) to be a divisor of source_size({})", if self.switch_level { "routers" } else { "servers" }, n, source_size);
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
			pool: vec![],//to be filled on initialization
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
	rng: Option<StdRng>,
}

impl Pattern for FixedRandom
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
	{
		self.map.reserve(source_size);
		let rng= self.rng.as_mut().unwrap_or(rng);
		for source in 0..source_size
		{
			// To avoid selecting self we subtract 1 from the total. If the random falls in the latter half we add it again.
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
		let mut rng = None;
		match_object_panic!(arg.cv,"FixedRandom",value,
			"seed" => rng = Some( value.as_rng().expect("bad value for seed") ),
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
		FixedRandom{
			map: vec![],//to be initialized
			allow_self,
			rng,
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
		assert_eq!(source_size, target_size, "source_size and target_size must be equal in IndependentRegions.");
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
		assert_eq!(self.sizes.iter().sum::<usize>(), source_size, "IndependentRegions sizes {:?} do not add up to the source_size {}", self.sizes, source_size);
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
		assert_eq!(patterns.len(), sizes.len().max(relative_sizes.len()), "Different number of entries in IndependentRegions.");
		IndependentRegions{
			patterns,
			sizes,
			relative_sizes,
		}
	}
}



/**
A pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying
some criteria. Note this is only a pattern, the actual packet route does not have to go through such middle router.
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
		assert_eq!(source_size, target_size, "The RestrictedMiddleUniform pattern needs source_size({})==target_size({})", source_size, target_size);
		assert_eq!(source_size % n, 0, "The RestrictedMiddleUniform pattern needs the number of {}({}) to be a divisor of source_size({})", if self.switch_level { "routers" } else { "servers" }, n, source_size);
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
	//initialized:
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
			pool: vec![],//to be filled on initialization
		}
	}
}

/**
Maps from a block into another following the natural embedding, keeping the coordinates of every node.
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
			panic!("Source sizes do not agree on CartesianEmbedding. source_size={source_size}, source_sides={sides:?}",source_size=source_size,sides=self.source_cartesian_data.sides);
		}
		if target_size!=self.destination_cartesian_data.size
		{
			panic!("Destination sizes do not agree on CartesianEmbedding. target_size={target_size}, destinations_sides={sides:?}",target_size=target_size,sides=self.destination_cartesian_data.sides);
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
Select a block in source/destination sets to send traffic according to a pattern and the remainder according to another. The `uncut_sides` parameter define a large block that may be the whole set, otherwise discarding elements from the end. The `cut_sides` parameter defines a subblock embedded in the former. This defines two sets of nodes: the ones in the subblock and the rest. A pattern can be provided for each of these two sets. It is possible to specify offsets and strides for the subblock.

For example, in a network with 150 servers we could do the following to see it as a `[3,10,5]` block with an `[3,4,3]` block embedded in it. The small block of 36 server selects destinations randomly inside it. The rest of the network, `150-36=114` servers also send randomly among themselves. No message is send between those two sets. The middle dimension has offset 1, so coordinates `[x,0,z]` are out of the small block. It has also stride 2, so it only includes odd `y` coordinates. More precisely, it includes those `[x,y,z]` with any `x`, `z<3`, and `y=2k+1` for `k<4`.
```ignore
CartesianCut{
	uncut_sides: [3,10,5],
	cut_sides: [3,4,3],
	cut_strides: [1,2,1],// defaults to a 1s vector
	cut_offsets: [0,1,0],// defaults to a 0s vector
	cut_pattern: Uniform,
	remainder_pattern: Uniform,//defaults to Identity
}
```
This same example would work for more than 150 servers, putting all that excess in the large set.

Another notable example is to combine several of them. Here, we use a decomposition of the previous whole `[3,10,5]` block into two disjoint blocks of size `[3,5,5]`. The offset is chosen to make sure of both being disjoint (a packing) and covering the whole. Then we select a pattern for each block. Since the two patterns are disjoint the can be [composed](Composition) to obtain a pattern that follows each of the blocks.
```ignore
Composition{patterns:[
	CartesianCut{
		uncut_sides: [3,10,5],
		cut_sides: [3,5,5],
		cut_offsets: [0,0,0],
		cut_pattern: RandomPermutation,
		//remainder_pattern: Identity,
	},
	CartesianCut{
		uncut_sides: [3,10,5],
		cut_sides: [3,5,5],
		cut_offsets: [0,5,0],
		cut_pattern: Uniform,
		//remainder_pattern: Identity,
	},
]}
```
**/
#[derive(Debug,Quantifiable)]
pub struct CartesianCut
{
	// /// An offset before the block.
	// start_margin: usize,
	// /// Some nodes out of the cube at the end.
	// end_margin: usize,
	/// The source sides. Any node beyond its size goes directly to the remained pattern.
	uncut_cartesian_data: CartesianData,
	/// The block we cut
	cut_cartesian_data: CartesianData,
	/// Offsets to set where the cut start at each dimension. Default to 0.
	cut_offsets: Vec<usize>,
	/// At each dimension cut 1 stripe for each `cut_stride[dim]` uncut cells. Default to 1.
	cut_strides: Vec<usize>,
	/// The pattern over the cut block.
	cut_pattern: Box<dyn Pattern>,
	/// The pattern over the rest.
	remainder_pattern: Box<dyn Pattern>,
}

impl Pattern for CartesianCut
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		let cut_size = self.cut_cartesian_data.size;
		self.cut_pattern.initialize(cut_size,cut_size,topology,rng);
		self.remainder_pattern.initialize(source_size-cut_size,target_size-cut_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let cut_size = self.cut_cartesian_data.size;
		if origin >= self.uncut_cartesian_data.size
		{
			let base = origin - cut_size;
			return self.from_remainder(self.remainder_pattern.get_destination(base,topology,rng));
		}
		let coordinates = self.uncut_cartesian_data.unpack(origin);
		let mut cut_count = 0;
		for dim in (0..coordinates.len()).rev()
		{
			if coordinates[dim] < self.cut_offsets[dim]
			{
				// Coordinate within margin
				return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
			}
			// how many 'rows' of cut are included.
			let hypercut_instances = (coordinates[dim] - self.cut_offsets[dim] + self.cut_strides[dim] -1 ) / self.cut_strides[dim];
			// the size of each 'row'.
			let hypercut_size : usize = self.cut_cartesian_data.sides[0..dim].iter().product();
			if hypercut_instances >= self.cut_cartesian_data.sides[dim]
			{
				// Beyond the cut
				cut_count += self.cut_cartesian_data.sides[dim]*hypercut_size;
				return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
			}
			cut_count += hypercut_instances*hypercut_size;
			if (coordinates[dim] - self.cut_offsets[dim]) % self.cut_strides[dim] != 0
			{
				// Space between stripes
				return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
			}
		}
		self.from_cut(self.cut_pattern.get_destination(cut_count,topology,rng))
	}
}

impl CartesianCut
{
	pub fn new(arg:PatternBuilderArgument) -> CartesianCut
	{
		let mut uncut_sides:Option<Vec<_>>=None;
		let mut cut_sides:Option<Vec<_>>=None;
		let mut cut_offsets:Option<Vec<_>>=None;
		let mut cut_strides:Option<Vec<_>>=None;
		let mut cut_pattern:Option<Box<dyn Pattern>>=None;
		let mut remainder_pattern:Option<Box<dyn Pattern>>=None;
		match_object_panic!(arg.cv,"CartesianCut",value,
			"uncut_sides" => uncut_sides = Some(value.as_array().expect("bad value for uncut_sides").iter()
				.map(|v|v.as_usize().expect("bad value in uncut_sides")).collect()),
			"cut_sides" => cut_sides = Some(value.as_array().expect("bad value for cut_sides").iter()
				.map(|v|v.as_usize().expect("bad value in cut_sides")).collect()),
			"cut_offsets" => cut_offsets = Some(value.as_array().expect("bad value for cut_offsets").iter()
				.map(|v|v.as_usize().expect("bad value in cut_offsets")).collect()),
			"cut_strides" => cut_strides = Some(value.as_array().expect("bad value for cut_strides").iter()
				.map(|v|v.as_usize().expect("bad value in cut_strides")).collect()),
			"cut_pattern" => cut_pattern = Some(new_pattern(arg.with_cv(value))),
			"remainder_pattern" => remainder_pattern = Some(new_pattern(arg.with_cv(value))),
		);
		let uncut_sides=uncut_sides.expect("There were no uncut_sides");
		let cut_sides=cut_sides.expect("There were no cut_sides");
		let n=uncut_sides.len();
		assert_eq!(n,cut_sides.len(),"CartesianCut: dimensions for uncut_sides and cut_sides must match.");
		let cut_offsets = cut_offsets.unwrap_or_else(||vec![0;n]);
		assert_eq!(n,cut_offsets.len(),"CartesianCut: dimensions for cut_offsets do not match.");
		let cut_strides = cut_strides.unwrap_or_else(||vec![1;n]);
		assert_eq!(n,cut_strides.len(),"CartesianCut: dimensions for cut_strides do not match.");
		let cut_pattern = cut_pattern.expect("There were no cut_pattern");
		let remainder_pattern = remainder_pattern.unwrap_or_else(||Box::new(Identity{}));
		CartesianCut{
			uncut_cartesian_data: CartesianData::new(&uncut_sides),
			cut_cartesian_data: CartesianData::new(&cut_sides),
			cut_offsets,
			cut_strides,
			cut_pattern,
			remainder_pattern,
		}
	}
	/**
	From an index in the cut region `(0..self.cut_cartesian_data.size)` get the whole index `0..target_size`.
	**/
	pub fn from_cut(&self, cut_index:usize) -> usize {
		//let hypercut_size : usize = self.cut_cartesian_data.sides[0..dim].iter().product();
		let n = self.cut_cartesian_data.sides.len();
		//let hpercut_sizes : Vec<usize> = Vec::with_capacity(n);
		//hypercut_sizes.push(1);
		//for dim in 1..n {
		//	hypercut_sizes.push( hypercut_sizes[dim-1]*self.cut_cartesian_data.sides[dim-1] );
		//}
		let coordinates = self.cut_cartesian_data.unpack(cut_index);
		let mut whole_index = 0;
		let mut hypersize = 1;
		for dim in 0..n {
			let coordinate = coordinates[dim]*self.cut_strides[dim] + self.cut_offsets[dim];
			whole_index += coordinate*hypersize;
			hypersize *= self.uncut_cartesian_data.sides[dim];
		}
		whole_index
	}
	/**
	From an index in the remainder region `(0..(target_size-self.cut_cartesian_data.size))` get the whole index.
	**/
	pub fn from_remainder(&self, remainder_index:usize) -> usize {
		if remainder_index >= self.uncut_cartesian_data.size {
			return remainder_index;
		}
		//let n = self.cut_cartesian_data.sides.len();
		//let hpercut_sizes : Vec<usize> = Vec::with_capacity(n);
		//hypercut_sizes.push(1);
		//for dim in 1..n {
		//	hypercut_sizes.push( hypercut_sizes[dim-1]*self.cut_cartesian_data.sides[dim-1] );
		//}
		//let mut whole_index = 0;
		////for dim in (0..n).rev() {
		////	let remaining_size = hyperuncut_sizes[dim]
		////	let coordinate = remainder_index - hypercut_sizes[dim];
		////	whole_index += coordinate*hypersize;
		////}
		//let mut remaining_size = 0;
		//for dim in 0..n {
		//	// Check if we are in this margin
		//	remaining_size += self.
		//}
		//whole_index
		todo!()
	}
}



/**
Apply some other [Pattern] over a set of nodes whose indices have been remapped according to a [Pattern]-given permutation.
A source `x` chooses as destination `map(pattern(invmap(x)))`, where `map` is the given permutation, `invmap` its inverse and `pattern` is the underlying pattern to apply. In other words, if `pattern(a)=b`, then destination of `map(a)` is set to `map(b)`. It can be seen as a [Composition] that manages building the inverse map.

Remapped nodes requires source and destination to be of the same size. The pattern creating the map is called once and must result in a permutation, as to be able to make its inverse.

For a similar operation on other types see [RemappedServersTopology](crate::topology::operations::RemappedServersTopology).

Example building a cycle in random order.
```ignore
RemappedNodes{
	/// The underlying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

**/
#[derive(Debug,Quantifiable)]
pub struct RemappedNodes
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
		let pattern = pattern.expect("There were no pattern in configuration of RemappedNodes.");
		let map = map.expect("There were no map in configuration of RemappedNodes.");
		RemappedNodes{
			from_base_map: vec![],
			into_base_map: vec![],
			pattern,
			map,
		}
	}
}


/**
Matrix by vector multiplication. Origin is given coordinates as within a block of size `source_size`.
Then the destination coordinate vector is `y=Mx`, with `x` being the origin and `M` the given `matrix`.
This destination vector is converted into an index into a block of size `target_size`.

If the parameter `check_admisible` is true, it will print a warning if the matrix given is not admissible.

Example configuration:
```ignore
LinearTransform{
	source_size: [4,8,8],
	matrix: [
		[1,0,0],
		[0,1,0],
		[0,0,1],
	],
	target_size: [4,8,8],
	legend_name: "Identity",
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct LinearTransform
{
	///The Cartesian interpretation for the source vector
	source_size: CartesianData,
	///A matrix of integers.
	matrix: Vec<Vec<i32>>,
	///The Cartesian interpretation for the destination vector
	target_size: CartesianData,
}

impl Pattern for LinearTransform
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!=self.source_size.size || target_size!=self.target_size.size
		{
			println!("source_size({})!=self.source_size.size({}) || target_size({})!=self.target_size.size({})",source_size,self.source_size.size,target_size,self.target_size.size);
			panic!("Sizes do not agree on LinearTransform.");
		}
		//Check that the number of lines of the matrix is the same as the number of dimensions.
		if self.matrix.len()!=self.target_size.sides.len()
		{
			panic!("The matrix has {} lines, but there are {} dimensions.",self.matrix.len(),self.target_size.sides.len());
		}
		//Check that the size of each line of the matrix is the same as the number of dimensions.
		for (index,line) in self.matrix.iter().enumerate()
		{
			if line.len()!=self.source_size.sides.len()
			{
				panic!("Line {} of the matrix has {} elements, but there are {} dimensions.",index,line.len(),self.source_size.sides.len());
			}
		}
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		//use std::convert::TryInto;
		let up_origin=self.source_size.unpack(origin);
		let mut result = vec![0usize;self.target_size.size];
		for (index,value) in self.matrix.iter().enumerate()
		{
			result[index] = (value.iter().zip(up_origin.iter()).map(|(&a, &b)| (a * b as i32)).sum::<i32>().rem_euclid(self.target_size.sides[index] as i32) ) as usize;
		}
		self.target_size.pack(&result)
	}
}

impl LinearTransform
{
	fn new(arg:PatternBuilderArgument) -> LinearTransform
	{
		let mut source_size:Option<Vec<_>>=None;
		let mut matrix:Option<Vec<Vec<i32>>>=None;
		let mut target_size:Option<Vec<_>>=None;
		let mut check_admisible = false;

		match_object_panic!(arg.cv,"LinearTransform",value,
			"source_size" => source_size = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"matrix" => matrix=Some(value.as_array().expect("bad value for matrix").iter()
				.map(|v|v.as_array().expect("bad value in matrix").iter().map(|n|n.as_i32().unwrap()).collect() ).collect() ),
			"target_size" => target_size = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"check_admisible" => check_admisible = value.as_bool().expect("bad value for check_admisible"),
		);
		let source_size=source_size.expect("There were no sides");
		let matrix=matrix.expect("There were no matrix");
		let target_size=target_size.expect("There were no sides");
		//let permute=permute.expect("There were no permute");
		//let complement=complement.expect("There were no complement");
		//calculate the derminant of the matrix
		if check_admisible{
			let determinant = laplace_determinant(&matrix);
			if determinant == 0
			{
				//print warning
				println!("WARNING: The determinant of the matrix in the LinearTransform is 0.");
			}
		}

		LinearTransform{
			source_size: CartesianData::new(&source_size),
			matrix,
			target_size: CartesianData::new(&target_size),
		}
	}
}


/**
Method to calculate the determinant of a matrix.

TODO: integrate in matrix.rs
 **/
fn laplace_determinant(matrix: &Vec<Vec<i32>>) -> i32
{
	let mut determinant = 0;
	if matrix.len() == 1
	{
		return matrix[0][0];
	}
	else if matrix.len() == 2
	{
		return matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
	}
	else
	{
		for i in 0..matrix.len()
		{
			let mut sub_matrix = vec![vec![0; matrix.len() - 1]; matrix.len() - 1];
			for j in 1..matrix.len()
			{
				let mut index = 0;
				for k in 0..matrix.len()
				{
					if k == i
					{
						continue;
					}
					sub_matrix[j - 1][index] = matrix[j][k];
					index += 1;
				}
			}
			determinant += matrix[0][i] * i32::pow(-1, i as u32) * laplace_determinant(&sub_matrix);
		}
	}
	determinant
}


/**
Use a `indexing` pattern to select among several possible patterns from the input to the output.
The `indexing` is initialized as a pattern from the input size to the number of `patterns`.
This is a Switch pattern, not a [Router] of packets.

This example keeps the even fixed and send odd input randomly. These odd input select even or odd indistinctly.
```ignore
Switch{
	indexing: LinearTansform{
		source_size: [2, 10],
		target_size: [2],
		matrix: [
			[1, 0],
		],
	},
	patterns: [
		Identity,
		Uniform,
	],
}
```

In this example the nodes at `(0,y)` are sent to a `(y,0,0)` row.
And the nodes at `(1,y)` are sent to a `(0,y,0)` column.
Destination `(0,0,0)` has both `(0,0)` and `(1,0)` as sources.
```ignore
Switch{
	indexing: LinearTransform{
		source_size: [2, 8],
		target_size: [2],
		matrix: [
			[1, 0],
		],
	},
	patterns: [
		Composition{patterns:[
			LinearTransform{
				source_size: [2, 8],
				target_size: [8],
				matrix: [
					[0, 1],
				],
			},
			CartesianEmbedding{
				source_sides: [8,1,1],
				destination_sides: [8,8,8],
			},
		],middle_sizes:[8]},
		Composition{patterns:[
			LinearTransform{
				source_size: [2, 8],
				target_size: [8],
				matrix: [
					[0, 1],
				],
			},
			CartesianEmbedding{
				source_sides: [1,8,1],
				destination_sides: [8,8,8],
			},
		],middle_sizes:[8]},
	],
},
```

TODO: describe `expand` and `seed`.

This example assigns 10 different RandomPermutations, depending on the `y` value, mentioned earlier.
```ignore
Switch{
	indexing: LinearTansform{
		source_size: [2, 10],
		target_size: [10],
		matrix: [
			[0, 1],
		],
	},
	patterns: [
		RandomPermutation,
	],
	expand: [10,],
}
```
**/
#[derive(Debug,Quantifiable)]
pub struct Switch {
	indexing: Box<dyn Pattern>,
	patterns: Vec<Box<dyn Pattern>>,
	seed: Option<f64>,
}

impl Pattern for Switch {
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.indexing.initialize(source_size,self.patterns.len(),topology,rng);

		let mut seed_generator = if let Some(seed) = self.seed{
			Some(StdRng::seed_from_u64(seed as u64))
		} else {
			None
		};
		for pattern in self.patterns.iter_mut() {
			if let Some( seed_generator) = seed_generator.as_mut(){
				let seed = seed_generator.next_u64();
				pattern.initialize(source_size,target_size,topology, &mut StdRng::seed_from_u64(seed));
			}else{
				pattern.initialize(source_size,target_size,topology, rng);
			}
		}
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		let index = self.indexing.get_destination(origin,topology,rng);
		self.patterns[index].get_destination(origin,topology,rng)
	}
}

impl Switch {
	fn new(arg:PatternBuilderArgument) -> Switch
	{
		let mut indexing = None;
		let mut patterns= None;//:Option<Vec<Box<dyn Pattern>>> = None;
		let mut expand: Option<Vec<usize>> = None;
		let mut seed = None;

		match_object_panic!(arg.cv,"Switch",value,
			"indexing" => indexing = Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"patterns" => patterns=Some( value.as_array().expect("bad value for patterns") ),
			"expand" => expand = Some(value.as_array().expect("bad value for expand").iter()
				.map(|v|v.as_usize().expect("bad value in expand")).collect()),
			"seed" => seed = Some(value.as_f64().expect("bad value for seed")),
		);
		let indexing = indexing.expect("Missing indexing in Switch.");
		let patterns = patterns.expect("Missing patterns in Switch.");
		let patterns = if let Some(expand) = expand {
			let mut new_patterns = vec![];
			for (index, pattern) in patterns.into_iter().enumerate() {
				for _ in 0..expand[index] {
					new_patterns.push(new_pattern(PatternBuilderArgument{cv:pattern,..arg}));
				}
			}
			new_patterns
		} else {
			patterns.iter().map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()
		};
		Switch{
			indexing,
			patterns,
            seed,
		}
	}
}

/**
```
	Uses the inverse of the pattern specified.
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Inverse
{
	///Pattern to apply.
	pattern: Box<dyn Pattern>,
	///Destination
	inverse_values: Vec<Option<usize>>,
	///default destination
	default_destination: Option<usize>,
}

impl Pattern for Inverse
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		// if source_size!= target_size
		// {
		// 	panic!("Inverse requires source and target sets to have same size.");
		// }
		self.pattern.initialize(source_size,target_size,_topology,_rng);
		let mut source = vec![None; source_size];
		for i in 0..source_size
		{
			let destination = self.pattern.get_destination(i,_topology,_rng);
			if let Some(_) = source[destination]
			{
				panic!("Inverse: destination {} is already used by origin {}.",destination,source[destination].unwrap());
			}
			source[destination] = Some(i);
		}
		self.inverse_values = source;
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		if origin >= self.inverse_values.len()
		{
			panic!("Inverse: origin {} is beyond the source size {}",origin,self.inverse_values.len());
		}
		if let Some(destination) = self.inverse_values[origin]
		{
			destination
		}
		else
		{
			self.default_destination.expect(&*("Inverse: origin ".to_owned() + &*origin.to_string() + " has no destination and there is no default destination."))
		}
	}
}

impl Inverse
{
	fn new(arg:PatternBuilderArgument) -> Inverse
	{
		let mut pattern = None;
		let mut default_destination = None;
		match_object_panic!(arg.cv,"Inverse",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"default_destination" => default_destination = Some(value.as_usize().expect("bad value for default_destination")),
		);
		let pattern = pattern.expect("There were no pattern in configuration of Inverse.");
		Inverse{
			pattern,
			inverse_values: vec![],
			default_destination,
		}
	}
}

/**

Select a region of tasks to execute a pattern. The size of the application using the pattern is 64.
```ignore
	SubApp{
		subtasks: 8,
		selection_pattern: CartesianEmbedding{
			source_sides: [1,8],
			destination_sides: [8,8],
		},
		subapp_pattern: CartesianTransform{
			sides: [8, 8],
			shift: [0, 1],
		},
		others_pattern: RandomPermutation,
	}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct SubApp
{
	subtasks: usize,
	selection_pattern: Box<dyn Pattern>,
	subapp_pattern: Box<dyn Pattern>,
	others_pattern: Box<dyn Pattern>,
	selected_vec: Vec<usize>,
}

impl Pattern for SubApp
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{

		if self.subtasks > source_size
		{
			panic!("SubApp: subtasks {} is greater than source size {}.",self.subtasks,source_size);
		}

		self.selection_pattern.initialize( self.subtasks, target_size, _topology, _rng);
		self.subapp_pattern.initialize(source_size,target_size,_topology,_rng);
		self.others_pattern.initialize(source_size,target_size,_topology,_rng);

		let mut source = vec![0; source_size];
		(0..self.subtasks).for_each(|i| {
			let destination = self.selection_pattern.get_destination(i,_topology,_rng);
			source[destination] = 1;
		});
		self.selected_vec = source;

	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		if self.selected_vec.len() <= origin
		{
			panic!("SubApp: origin {} is beyond the source size {}",origin,self.selected_vec.len());
		}

		if self.selected_vec[origin] == 1
		{
			self.subapp_pattern.get_destination(origin,_topology,_rng)
		}
		else
		{
			self.others_pattern.get_destination(origin,_topology,_rng)
		}

	}
}

impl SubApp
{
	fn new(arg:PatternBuilderArgument) -> SubApp
	{
		let mut subtasks = None;
		let mut selection_pattern = None;
		let mut subapp_pattern = None;
		let mut others_pattern = None;
		match_object_panic!(arg.cv,"SubApp",value,
			"subtasks" => subtasks = Some(value.as_usize().expect("bad value for total_subsize")),
			"selection_pattern" => selection_pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})), //map of the application over the machine
			"subapp_pattern" => subapp_pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})), //traffic of the application
			"others_pattern" => others_pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})), //traffic of the machine
		);

		let subtasks = subtasks.expect("There were no tasks in configuration of SubApp.");
		let subapp_pattern = subapp_pattern.expect("There were no subapp_pattern in configuration of SubApp.");
		let selection_pattern = selection_pattern.expect("There were no selection_pattern in configuration of SubApp.");
		let others_pattern = others_pattern.expect("There were no others_pattern in configuration of SubApp.");

		SubApp{
			subtasks,
			subapp_pattern,
			selection_pattern,
			others_pattern,
			selected_vec: vec![],
		}

	}
}


/**
For each source, it keeps a state of the last destination used. When applying the pattern, it uses the last destination as the origin for the pattern, and
the destination is saved for the next call to the pattern.
```ignore
ElementComposition{
	pattern: RandomPermutation,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ElementComposition
{
	///Pattern to apply.
	pattern: Box<dyn Pattern>,
	///Pending destinations.
	origin_state: RefCell<Vec<usize>>,
}

impl Pattern for ElementComposition
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!= target_size
		{
			panic!("ElementComposition requires source and target sets to have same size.");
		}
		self.pattern.initialize(source_size,target_size,_topology,_rng);
		self.origin_state.replace((0..source_size).collect());
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		if origin >= self.origin_state.borrow().len()
		{
			panic!("ElementComposition: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
		}
		let index = self.origin_state.borrow_mut()[origin];
		let destination = self.pattern.get_destination(index,_topology,rng);
		self.origin_state.borrow_mut()[origin] = destination;
		destination
	}
}

impl ElementComposition
{
	fn new(arg:PatternBuilderArgument) -> ElementComposition
	{
		let mut pattern = None;
		match_object_panic!(arg.cv,"ElementComposition",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
		);
		let pattern = pattern.expect("There were no pattern in configuration of ElementComposition.");
		ElementComposition{
			pattern,
			origin_state: RefCell::new(vec![]),
		}
	}
}
/**
* Pattern which simulates an all-gather or all-reduce in log p steps, applying the recursive doubling technique.
* The communications represent a Hypercube.
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RecursiveDistanceHalving
{
	///Pending destinations.
	origin_state: RefCell<Vec<usize>>,
	///Map for the different states
	cartesian_data: CartesianData,
	///Order of the neighbours
	neighbours_order: Option<Vec<Vec<usize>>>,
}

impl Pattern for RecursiveDistanceHalving
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!= target_size
		{
			panic!("RecursiveDistanceHalving requires source and target sets to have same size.");
		}
		//If the source size is not a power of 2, the pattern will not work.
		if !source_size.is_power_of_two()
		{
			panic!("RecursiveDistanceHalving requires source size to be a power of 2.");
		}
		let pow = source_size.ilog2();
		self.origin_state = RefCell::new(vec![0;source_size]);
		self.cartesian_data = CartesianData::new(&(vec![2; pow as usize]))//(0..pow).map(|i| CartesianData::new(&[source_size/2_usize.pow(i), 2_usize.pow(i)]) ).collect();
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		if origin >= self.origin_state.borrow().len()
		{
			panic!("RecursiveDistanceHalving: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
		}
		let index = self.origin_state.borrow()[origin];
		if index >=self.cartesian_data.sides.len()
		{
			return origin; //No more to do...
		}

		let mut state = self.origin_state.borrow_mut();
		let source_coord = self.cartesian_data.unpack(origin);
		let to_send = if let Some(vectores) = self.neighbours_order.as_ref()
		{
			vectores[state[origin]].clone()
		}else {
			self.cartesian_data.unpack(2_i32.pow(state[origin].try_into().unwrap()) as usize)
		};

		let dest = source_coord.iter().zip(to_send.iter()).map(|(a,b)| a^b).collect::<Vec<usize>>();
		state[origin]+=1;
		self.cartesian_data.pack(&dest)

	}
}

impl RecursiveDistanceHalving
{
	fn new(arg:PatternBuilderArgument) -> RecursiveDistanceHalving
	{
		let mut neighbours_order: Option<Vec<usize>> = None; //Array of vectors which represent the order of the neighbours
		match_object_panic!(arg.cv,"RecursiveDistanceHalving",value,
			"neighbours_order" => neighbours_order = Some(value.as_array().expect("bad value for neighbours_order").iter()
				.map(|n|n.as_usize().unwrap()).collect() ),
		);

		//now each number in the array transform it into an array of binary numbers
		let binary_order = if let Some(n) = neighbours_order
		{
			//get the biggest number
			let max = n.iter().max().unwrap();
			//calculate the number of bits
			let bits = max.ilog2() as usize + 1usize;
			//transform each number into a binary number with the same number of bits
			let bin_n = n.iter().map(|&x| {
				let mut v = vec![0; bits];
				let mut x = x;
				for i in 0..bits
				{
					v[i] = x%2;
					x = x/2;
				}
				v
			}).collect();
			Some(bin_n)

		}else{
			None
		};

		RecursiveDistanceHalving{
			origin_state: RefCell::new(vec![]),
			cartesian_data: CartesianData::new(&vec![0;0]),
			neighbours_order: binary_order,
		}
	}
}


/**
* Pattern to simulate communications in a BinomialTree.
* Going upwards could be seen as a reduction, and going downwards as a broadcast.
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct BinomialTree
{
	///How to go through the tree.
	upwards: bool,
	///Tree embedded into a Hypercube
	cartesian_data: CartesianData,
	///State indicating the neighbour to send downwards
	state: RefCell<Vec<usize>>,
}

impl Pattern for BinomialTree
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		if source_size!= target_size
		{
			panic!("BinomialTree requires source and target sets to have same size.");
		}

		if !source_size.is_power_of_two()
		{
			panic!("BinomialTree requires source size to be a power of 2.");
		}

		let mut tree_order = source_size.ilog2();

		if source_size > 2usize.pow(tree_order)
		{
			tree_order +=1;
		}
		self.cartesian_data = CartesianData::new(&vec![2; tree_order as usize]); // Tree emdebbed into an hypercube
		self.state = RefCell::new(vec![0; source_size]);
    }
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		if origin >= self.cartesian_data.size
		{
			panic!("BinomialTree: origin {} is beyond the source size {}",origin,self.cartesian_data.size);
		}
		let mut source_coord = self.cartesian_data.unpack(origin);
		let first_one_index = source_coord.iter().enumerate().find(|(_index, &value)| value == 1);

		return if self.upwards
		{
			if origin == 0 {
				0
			} else {
				let first_one_index = first_one_index.unwrap().0;
				let state = self.state.borrow()[origin];
				if state == 1{
					origin
				}else{
					self.state.borrow_mut()[origin] = 1;
					source_coord[first_one_index] = 0;
					self.cartesian_data.pack(&source_coord)
				}
			}
		}else{
			let first_one_index = if origin == 0{
					self.cartesian_data.sides.len() //log x in base 2... the number of edges in hypercube
				} else{
					first_one_index.unwrap().0
				};
			let son_index = self.state.borrow()[origin];

			if first_one_index > son_index
			{
				self.state.borrow_mut()[origin] += 1;
				origin + 2usize.pow(son_index as u32)
			}else{
				origin // no sons / no more sons to send
			}
		}
	}
}

impl BinomialTree
{
	fn new(arg:PatternBuilderArgument) -> BinomialTree
	{
		let mut upwards = None;
		match_object_panic!(arg.cv,"BinomialTree",value,
			"upwards" => upwards = Some(value.as_bool().expect("bad value for upwards for pattern BinomialTree")),
		);
		let upwards = upwards.expect("There were no upwards in configuration of BinomialTree.");
		BinomialTree{
			upwards,
			cartesian_data: CartesianData::new(&vec![2;2]),
			state: RefCell::new(vec![]),
		}
	}
}



/**
Boolean function which puts a 1 if the pattern contains the server, and 0 otherwise.
```ignore
BooleanFunction{
	pattern: Hotspots{selected_destinations: [0]}, //1 if the server is 0, 0 otherwise
	pattern_destination_size: 1,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CandidatesSelection
{
	///Pattern to apply.
	selected: Option<Vec<usize>>,
	///Pattern to apply.
	pattern: Box<dyn Pattern>,
	///Pattern destination size.
	pattern_destination_size: usize,
}

impl Pattern for CandidatesSelection
{
	fn initialize(&mut self, source_size:usize, _target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
	{
		// if target_size != 2
		// {
		// 	panic!("CandidatesSelection requires target size to be 2.");
		// }
		self.pattern.initialize(source_size, self.pattern_destination_size, _topology, _rng);
		let mut selection = vec![0;source_size];
		for i in 0..source_size
		{
			selection[self.pattern.get_destination(i,_topology,_rng)] = 1;
		}
		self.selected = Some(selection);
	}
	fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		if origin >= self.selected.as_ref().unwrap().len()
		{
			panic!("CandidatesSelection: origin {} is beyond the source size {}",origin,self.selected.as_ref().unwrap().len());
		}
		self.selected.as_ref().unwrap()[origin]
	}
}

impl CandidatesSelection
{
	fn new(arg:PatternBuilderArgument) -> CandidatesSelection
	{
		let mut pattern = None;
		let mut pattern_destination_size = None;
		match_object_panic!(arg.cv,"CandidatesSelection",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"pattern_destination_size" => pattern_destination_size = Some(value.as_usize().expect("bad value for pattern_destination_size")),
		);
		let pattern = pattern.expect("There were no pattern in configuration of CandidatesSelection.");
		let pattern_destination_size = pattern_destination_size.expect("There were no pattern_destination_size in configuration of CandidatesSelection.");
		CandidatesSelection{
			selected: None,
			pattern,
			pattern_destination_size,
		}
	}
}
/**
A transparent meta-pattern to help debug other [Pattern].

```ignore
Debug{
	pattern: ...,
	check_permutation: true,
}
```
**/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
struct DebugPattern {
	/// The pattern being applied transparently.
	pattern: Box<dyn Pattern>,
	/// Whether to consider an error not being a permutation.
	check_permutation: bool,
	/// Size of source cached at initialization.
	source_size: usize,
	/// Size of target cached at initialization.
	target_size: usize,
}

impl Pattern for DebugPattern{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.source_size = source_size;
		self.target_size = target_size;
		self.pattern.initialize(source_size,target_size,topology,rng);
		if self.check_permutation {
			if source_size != target_size {
				panic!("cannot be a permutation is source size {} and target size {} do not agree.",source_size,target_size);
			}
			let mut hits = vec![false;target_size];
			for origin in 0..source_size {
				let dst = self.pattern.get_destination(origin,topology,rng);
				if hits[dst] {
					panic!("Destination {} hit at least twice.",dst);
				}
				hits[dst] = true;
			}
		}
	}
	fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
	{
		if origin >= self.source_size {
			panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
		}
		let dst = self.pattern.get_destination(origin,topology,rng);
		if dst >= self.target_size {
			panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
		}
		dst
	}
}

impl DebugPattern{
	fn new(arg:PatternBuilderArgument) -> DebugPattern{
		let mut pattern = None;
		let mut check_permutation = false;
		match_object_panic!(arg.cv,"Debug",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
		);
		let pattern = pattern.expect("Missing pattern in configuration of Debug.");
		DebugPattern{
			pattern,
			check_permutation,
			source_size:0,
			target_size:0,
		}
	}
}

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct EncapsulatedPattern {}

impl EncapsulatedPattern {
	fn new(pattern: String, arg:PatternBuilderArgument) -> Box<dyn Pattern> {
		let pattern_cv = match pattern.as_str(){
			"Stencil" =>{
				let mut task_space = None;
				match_object_panic!(arg.cv,"Stencil",value,
					"task_space" => task_space = Some(value.as_array().expect("bad value for task_space").iter()
						.map(|v|v.as_usize().expect("bad value in task_space")).collect()),
				);
				let task_space = task_space.expect("There were no task_space in configuration of Stencil.");
				Some(get_stencil(task_space))
			},
			_ => panic!("Pattern {} not found.",pattern),
		};
		new_pattern(PatternBuilderArgument{cv:&pattern_cv.unwrap(),..arg})
	}
}

fn get_stencil(task_space: Vec<usize>) -> ConfigurationValue
{
	let space_cv = ConfigurationValue::Array(task_space.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

	let mut transforms = vec![];
	for i in 0..task_space.len()
	{
		let mut transform_suc = vec![0;task_space.len()]; //next element in dimension
		transform_suc[i] = 1;
		let transform_suc_cv = ConfigurationValue::Array(transform_suc.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

		let mut transform_pred = vec![0;task_space.len()]; //previous element in dimension
		transform_pred[i] = (task_space[i] -1).rem_euclid(task_space[i]);
		let transform_pred_cv = ConfigurationValue::Array(transform_pred.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

		transforms.push(
			ConfigurationValue::Object("CartesianTransform".to_string(), vec![
				("sides".to_string(), space_cv.clone()),
				("shift".to_string(), transform_suc_cv),
			]),
		);

		transforms.push(
			ConfigurationValue::Object("CartesianTransform".to_string(), vec![
				("sides".to_string(), space_cv.clone()),
				("shift".to_string(), transform_pred_cv),
			]),
		);
	}

	ConfigurationValue::Object( "RoundRobin".to_string(), vec![
		("patterns".to_string(), ConfigurationValue::Array(transforms)),
	])
}


pub fn get_switch_pattern(index_pattern: ConfigurationValue, patterns: Vec<ConfigurationValue>) -> ConfigurationValue{
	ConfigurationValue::Object("Switch".to_string(), vec![
		("indexing".to_string(), index_pattern),
		("patterns".to_string(), ConfigurationValue::Array(patterns)),
	])
}

pub fn get_candidates_selection(pattern: ConfigurationValue, pattern_destination_size: usize) -> ConfigurationValue{
	ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
		("pattern".to_string(), pattern),
		("pattern_destination_size".to_string(), ConfigurationValue::Number(pattern_destination_size as f64)),
	])
}

pub fn get_cartesian_transform(sides: Vec<usize>, shift: Option<Vec<usize>>, patterns: Option<Vec<ConfigurationValue>>) -> ConfigurationValue{
	let mut config = vec![
		("sides".to_string(), ConfigurationValue::Array(sides.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())),
	];
	if let Some(shift) = shift{
		config.push(("shift".to_string(), ConfigurationValue::Array(shift.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())));
	}
	if let Some(patterns) = patterns{
		config.push(("patterns".to_string(), ConfigurationValue::Array(patterns)));
	}
	ConfigurationValue::Object("CartesianTransform".to_string(), config)
}

pub fn get_hotspot_destination(selected_destinations: Vec<usize>) -> ConfigurationValue{
	ConfigurationValue::Object("Hotspots".to_string(), vec![
		("destinations".to_string(), ConfigurationValue::Array(selected_destinations.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>()), )
	])
}


/**
A transparent meta-pattern to help debug other [Pattern].

```ignore
Debug{
	pattern: ...,
	check_permutation: true,
}
```
 **/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
struct MiDebugPattern {
	/// The pattern being applied transparently.
	pattern: Vec<Box<dyn Pattern>>,
	/// Whether to consider an error not being a permutation.
	check_permutation: bool,
	/// Whether to consider an error not being an injection.
	check_injective: bool,
	/// Size of source cached at initialization.
	source_size: Vec<usize>,
	/// Size of target cached at initialization.
	target_size: usize,
}

impl Pattern for MiDebugPattern {
	fn initialize(&mut self, _source_size:usize, _target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
	{
		// self.source_size = source_size;
		// self.target_size = target_size;
		for (index, pattern) in self.pattern.iter_mut().enumerate() {
			pattern.initialize(self.source_size[index], self.target_size, topology,rng);
		}

		if self.check_injective{

			if self.source_size.iter().sum::<usize>() > self.target_size{
				panic!("cannot be injective if source size {} is more than target size {}",self.source_size.iter().sum::<usize>(),self.target_size);
			}
			let mut hits = vec![-1;self.target_size];
			for (index, size) in self.source_size.iter().enumerate() {

				for origin_local in 0..*size {
					let dst = self.pattern[index].get_destination(origin_local,topology,rng);
					if hits[dst] != -1 {
						panic!("Destination {} hit by origin {}, now by {}, in pattern: {}",dst,hits[dst],origin_local, index);
					}
					hits[dst] = origin_local as isize;
				}

			}
			println!("Check injective patterns passed.");
			println!("There were the following number of sources: {:?} ({}), and the following number of destinations: {}",self.source_size,self.source_size.iter().sum::<usize>(),self.target_size);
			println!("There are {} free destinations, and {} servers hits. The free destinations are: {:?}",hits.iter().filter(|x|**x==-1).count(),hits.iter().filter(|x|**x!=-1).count(),hits.iter().enumerate().filter(|(_,x)|**x==-1).map(|(i,_)|i).collect::<Vec<usize>>());

		}
		// if self.check_permutation {
		// 	if self.source_size != self.target_size {
		// 		panic!("cannot be a permutation is source size {} and target size {} do not agree.",self.source_size,self.target_size);
		// 	}
		// 	let mut hits = vec![false;self.target_size];
		// 	for origin in 0..self.source_size {
		// 		let dst = self.pattern.get_destination(origin,topology,rng);
		// 		if hits[dst] {
		// 			panic!("Destination {} hit at least twice.",dst);
		// 		}
		// 		hits[dst] = true;
		// 	}
		// }
		panic!("This is just a check.")
	}
	fn get_destination(&self, _origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
	{
		0
		// if origin >= self.source_size {
		// 	panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
		// }
		// let dst = self.pattern.get_destination(origin,topology,rng);
		// if dst >= self.target_size {
		// 	panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
		// }
		// dst
	}
}

impl MiDebugPattern {
	fn new(arg:PatternBuilderArgument) -> MiDebugPattern {
		let mut pattern = None;
		let mut check_permutation = false;
		let mut check_injective = false;
		let mut source_size = None;
		let mut target_size = None;
		match_object_panic!(arg.cv,"Debug",value,
			"patterns" => pattern = Some(value.as_array().expect("bad value for pattern").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
			"source_size" => source_size = Some(value.as_array().expect("bad value for source_size").iter()
				.map(|v|v.as_usize().expect("bad value in source_size")).collect()),
			"target_size" => target_size = Some(value.as_usize().expect("bad value for target_size")),
			"check_injective" => check_injective = value.as_bool().expect("bad value for check_injective"),
		);
		let pattern = pattern.expect("Missing pattern in configuration of Debug.");
		let source_size = source_size.expect("Missing source_size in configuration of Debug.");
		let target_size = target_size.expect("Missing target_size in configuration of Debug.");
		MiDebugPattern {
			pattern,
			check_permutation,
			check_injective,
			source_size,
			target_size,
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
			assert_eq!(count, 0, "Got {} selfs at size {}.", count, size);
		}
	}
}

