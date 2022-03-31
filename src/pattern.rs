/*!

A Pattern defines the way elements select their destinations.

see [`new_pattern`](fn.new_pattern.html) for documentation on the configuration syntax of predefined patterns.

*/

use std::cell::{RefCell};
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom};
use std::fs::File;
use std::io::{BufRead,BufReader};
use std::ops::DerefMut;

use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology,Location};
use crate::quantify::Quantifiable;
use crate::{Plugs,match_object_panic};

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
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>);
	///Obtain a destination of a source. This will be called repeteadly as the traffic requires destination for its messages.
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize;
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

In the uniform pattern all elements have same probability to send to any other.
```ignore
Uniform{
	legend_name: "uniform",
}
```

### GloballyShufflingDestinations

An uniform-like pattern that avoids repeating the same destination. It keeps a global vector of destinations. It is shuffled and each created message gets its destination from there. Sometimes you may be selected yourself as destination.

```ignore
GloballyShufflingDestinations{
	legend_name: "globally shuffled destinations",
}
```

### GroupShufflingDestinations

Alike `GloballyShufflingDestinations` but keeping one destination vector per each group.

```ignore
GroupShufflingDestinations{
	//E.g., if we select `group_size` to be the number of servers per router we are keeping a destination vector for each router.
	group_size: 5,
	legend_name: "router shuffled destinations",
}
```

### UniformDistance

Each message gets its destination uniformly random among the servers attached to neighbour routers.

```ignore
UniformDistance{
	distance: 1,
	legend_name: "uniform among neighbours",
}
```


## Permutations and maps.
Each element has a unique destination and a unique element from which it is a destination.

### RandomPermutation
Have same chance to generate any permutation
```
RandomPermutation{
	legend_name: "random server permutation",
}
```

### RandomInvolution
Can only generate involutions. This is, if `p` is the permutation then for any element `x`, `p(p(x))=x`.
```
RandomInvolution{
	legend_name: "random server involution",
}
```

### FixedRandom
Each source has an independent unique destination. By the "birtday paradox" we can expect several sources to share a destination, causing incast contention.

### FileMap
A map read from file. Each elment has a unique destination.
```
FileMap{
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```

### CartesianTransform
Sees the elments as a n-dimensional orthohedra. Then it applies several transformations. When mapping directly servers it may be useful to use as `sides[0]` the number of servers per router.
```
CartesianTransform{
	sides: [4,8,8],
	shift: [0,4,0],//optional
	permute: [0,2,1],//optional
	complement: [false,true,false],//optional
	project: [false,false,false],//optional
	legend_name: "Some lineal transformation over a 8x8 mesh with 4 servers per router",
}
```

### Hotspots.
A pool of hotspots is build from a given list of `destinations` plus some amount `extra_random_destinations` computed randomly on initialization.
Destinations are randomly selected from such pool.
This causes incast contention, more explicitly than `FixedRandom`.
```
Hotspots{
	//destinations: [],//default empty
	extra_random_destinations: 5,//default 0
	legend_name: "every server send to one of 5 randomly selected hotspots",
}
```

## meta patterns

### Product
The elements are divided in blocks. Blocks are mapped to blocks by the `global_pattern`. The `block_pattern` must has input and output size equal to `block_size` and maps the specific elements.
```
Product{
	block_pattern: RandomPermutation,
	global_pattern: RandomPermutation,
	block_size: 10,
	legend_name:"permutation of blocks",
}
```

### Component
Divides the topology along link classes. The 'local' pattern is Uniform.
```
Components{
	global_pattern: RandomPermutation,
	component_classes: [0],
	legend_name: "permutation of the induced group by the 0 link class",
}
```

### Composition
Allows to concatenate transformations.
```
Composition{
	patterns: [  FileMap{filename: "/patterns/second"}, FileMap{filename: "/patterns/first"}  ]
	legend_name: "Apply first to origin, and then second to get the destination",
}
```


### Pow.
A Pow is composition of a `pattern` with itself `exponent` times.
```
Pow{
	pattern: FileMap{filename: "/patterns/mypattern"},
	exponent: "3",
	legend_name: "Apply 3 times my pattern",
}
```


### RandomMix
Probabilistically mix a list of patterns.
```
RandomMix{
	patterns: [Hotspots{extra_random_destination:10}, Uniform],
	weight: [5,95],
	legend_name: "0.05 chance of sending to the hotspots",
}
```

*/
pub fn new_pattern(arg:PatternBuilderArgument) -> Box<dyn Pattern>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		match arg.plugs.patterns.get(cv_name)
		{
			Some(builder) => return builder(arg),
			_ => (),
		};
		match cv_name.as_ref()
		{
			"Identity" => Box::new(Identity::new(arg)),
			"Uniform" => Box::new(UniformPattern::new(arg)),
			"RandomPermutation" => Box::new(RandomPermutation::new(arg)),
			"RandomInvolution" => Box::new(RandomInvolution::new(arg)),
			"FileMap" => Box::new(FileMap::new(arg)),
			"Product" => Box::new(ProductPattern::new(arg)),
			"Components" => Box::new(ComponentsPattern::new(arg)),
			"CartesianTransform" => Box::new(CartesianTransform::new(arg)),
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
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		if source_size!=target_size
		{
			unimplemented!("The Identity pattern requires source_size({})=target_size({})",source_size,target_size);
		}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
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
///TODO: for some meta-patterns it would be useful to allow self-messages.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformPattern
{
	size: usize,
}

//impl Quantifiable for UniformPattern
//{
//	fn total_memory(&self) -> usize
//	{
//		return size_of::<Self>();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl Pattern for UniformPattern
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		self.size=target_size;
		if source_size!=target_size
		{
			unimplemented!("Different sizes are not yet implemented for UniformPattern");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		//let mut rng = thread_rng();//FIXME use seed
		loop
		{
			//let r=rng.borrow_mut().gen_range(0,self.size);//in rand-0.4
			let r=rng.borrow_mut().gen_range(0..self.size);//in rand-0.8
			if r!=origin
			{
				return r;
			}
		}
	}
}

impl UniformPattern
{
	fn new(arg:PatternBuilderArgument) -> UniformPattern
	{
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Uniform"
			{
				panic!("A UniformPattern must be created from a `Uniform` object not `{}`",cv_name);
			}
			for &(ref name,ref _value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"legend_name" => (),
					//"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					//"servers" => match value
					//{
					//	&ConfigurationValue::Number(f) => servers=Some(f as usize),
					//	_ => panic!("bad value for servers"),
					//}
					//"load" => match value
					//{
					//	&ConfigurationValue::Number(f) => load=Some(f as f32),
					//	_ => panic!("bad value for load"),
					//}
					//"message_size" => (),
					_ => panic!("Nothing to do with field {} in UniformPattern",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a UniformPattern from a non-Object");
		}
		UniformPattern{
			size:0,
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
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		if source_size!=target_size
		{
			panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		self.permutation=(0..source_size).collect();
		//rng.borrow_mut().shuffle(&mut self.permutation);//rand-0.4
		self.permutation.shuffle(rng.borrow_mut().deref_mut());//rand-0.8
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
	{
		self.permutation[origin]
	}
}

impl RandomPermutation
{
	fn new(arg:PatternBuilderArgument) -> RandomPermutation
	{
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="RandomPermutation"
			{
				panic!("A RandomPermutation must be created from a `RandomPermutation` object not `{}`",cv_name);
			}
			for &(ref name,ref _value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomPermutation",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a RandomPermutation from a non-Object");
		}
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

//impl Quantifiable for RandomInvolution
//{
//	fn total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl Pattern for RandomInvolution
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		if source_size!=target_size
		{
			panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		//self.permutation=(0..source_size).collect();
		//rng.borrow_mut().shuffle(&mut self.permutation);
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
			let first=rng.borrow_mut().gen_range(0..max);
			let second=rng.borrow_mut().gen_range(0..max-1);
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
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
	{
		self.permutation[origin]
	}
}

impl RandomInvolution
{
	fn new(arg:PatternBuilderArgument) -> RandomInvolution
	{
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="RandomInvolution"
			{
				panic!("A RandomInvolution must be created from a `RandomInvolution` object not `{}`",cv_name);
			}
			for &(ref name,ref _value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomInvolution",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a RandomInvolution from a non-Object");
		}
		RandomInvolution{
			permutation: vec![],
		}
	}
}


///Use a permutation given via a file.
///See `RandomPermutation`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FileMap
{
	permutation: Vec<usize>,
}

//impl Quantifiable for FileMap
//{
//	fn total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl Pattern for FileMap
{
	fn initialize(&mut self, _source_size:usize, _target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		//self.permutation=(0..size).collect();
		//rng.borrow_mut().shuffle(&mut self.permutation);
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
	{
		self.permutation[origin]
	}
}

impl FileMap
{
	fn new(arg:PatternBuilderArgument) -> FileMap
	{
		let mut filename=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="FileMap"
			{
				panic!("A FileMap must be created from a `FileMap` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"filename" => match value
					{
						&ConfigurationValue::Literal(ref s) => filename=Some(s.to_string()),
						_ => panic!("bad value for filename"),
					},
					//"pattern" => pattern=Some(new_pattern(value)),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in FileMap",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a FileMap from a non-Object");
		}
		let filename=filename.expect("There were no filename");
		let file=File::open(&filename).expect("could not open pattern file.");
		let reader = BufReader::new(&file);
		let mut lines=reader.lines();
		let mut permutation=Vec::new();
		while let Some(rline)=lines.next()
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
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		if source_size!=target_size
		{
			unimplemented!("Different sizes are not yet implemented for ProductPattern");
		}
		self.block_pattern.initialize(self.block_size,self.block_size,topology,rng);
		let global_size=source_size/self.block_size;
		self.global_pattern.initialize(global_size,global_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
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
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Product"
			{
				panic!("A ProductPattern must be created from a `Product` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"block_pattern" => block_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					"block_size" => match value
					{
						&ConfigurationValue::Number(f) => block_size=Some(f as usize),
						_ => panic!("bad value for block_size"),
					}
					//"load" => match value
					//{
					//	&ConfigurationValue::Number(f) => load=Some(f as f32),
					//	_ => panic!("bad value for load"),
					//}
					//"message_size" => (),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in ProductPattern",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a ProductPattern from a non-Object");
		}
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
	fn initialize(&mut self, _source_size:usize, _target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
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
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
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
		let r_local=rng.borrow_mut().gen_range(0..self.components[global_dest].len());
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
		let rserver=rng.borrow_mut().gen_range(0..candidate_stack.len());
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
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Components"
			{
				panic!("A ComponentsPattern must be created from a `Components` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"block_pattern" => block_pattern=Some(new_pattern(value,plugs)),
					"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					"component_classes" => match value
					{
						&ConfigurationValue::Array(ref a) => component_classes=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in component_classes"),
						}).collect()),
						_ => panic!("bad value for component_classes"),
					}
					//"load" => match value
					//{
					//	&ConfigurationValue::Number(f) => load=Some(f as f32),
					//	_ => panic!("bad value for load"),
					//}
					//"message_size" => (),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in ComponentsPattern",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a ComponentsPattern from a non-Object");
		}
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


/// Interpretate the origin as with cartesian coordinates and apply transformations.
/// May permute the dimensions if they have same side.
/// May complement the dimensions.
/// Order of composition is: first shift, second permute, third complement, fourth project.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianTransform
{
	///The Cartesian interpretation.
	cartesian_data: CartesianData,
	///A shift to each coordinate, modulo the side.
	shift: Option<Vec<usize>>,
	///Optionally how dimensions are permuted.
	///`permute=[0,2,1]` means to permute dimensions 1 and 2, keeping dimension 0 as is.
	permute: Option<Vec<usize>>,
	///Optionally, which dimensions must be complemented.
	///`complement=[true,false,false]` means `target_coordinates[0]=side-1-coordinates[0]`.
	complement: Option<Vec<bool>>,
	///Indicates dimensions to be projected into 0. This causes incast contention.
	project: Option<Vec<bool>>,
}

impl Pattern for CartesianTransform
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		if source_size!=target_size
		{
			panic!("In a Cartesiantransform source_size({}) must be equal to target_size({}).",source_size,target_size);
		}
		if source_size!=self.cartesian_data.size
		{
			panic!("Sizes do not agree on CartesianTransform.");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
	{
		let up_origin=self.cartesian_data.unpack(origin);
		let up_shifted=match self.shift
		{
			Some(ref v) => v.iter().enumerate().map(|(index,&value)|(up_origin[index]+value)%self.cartesian_data.sides[index]).collect(),
			None => up_origin,
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
		self.cartesian_data.pack(&up_projected)
	}
}

impl CartesianTransform
{
	fn new(arg:PatternBuilderArgument) -> CartesianTransform
	{
		let mut sides=None;
		let mut shift=None;
		let mut permute=None;
		let mut complement=None;
		let mut project=None;
		//if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		//{
		//	if cv_name!="CartesianTransform"
		//	{
		//		panic!("A CartesianTransform must be created from a `CartesianTransform` object not `{}`",cv_name);
		//	}
		//	for &(ref name,ref value) in cv_pairs
		//	{
		//		//match name.as_ref()
		//		match AsRef::<str>::as_ref(&name)
		//		{
		//			"sides" => match value
		//			{
		//				&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
		//					&ConfigurationValue::Number(f) => f as usize,
		//					_ => panic!("bad value in sides"),
		//				}).collect()),
		//				_ => panic!("bad value for sides"),
		//			}
		//			"shift" => match value
		//			{
		//				&ConfigurationValue::Array(ref a) => shift=Some(a.iter().map(|v|match v{
		//					&ConfigurationValue::Number(f) => f as usize,
		//					_ => panic!("bad value in shift"),
		//				}).collect()),
		//				_ => panic!("bad value for shift"),
		//			}
		//			"permute" => match value
		//			{
		//				&ConfigurationValue::Array(ref a) => permute=Some(a.iter().map(|v|match v{
		//					&ConfigurationValue::Number(f) => f as usize,
		//					_ => panic!("bad value in permute"),
		//				}).collect()),
		//				_ => panic!("bad value for permute"),
		//			}
		//			"complement" => match value
		//			{
		//				&ConfigurationValue::Array(ref a) => complement=Some(a.iter().map(|v|match v{
		//					&ConfigurationValue::True => true,
		//					&ConfigurationValue::False => false,
		//					_ => panic!("bad value in complement"),
		//				}).collect()),
		//				_ => panic!("bad value for complement"),
		//			}
		//			"project" => match value
		//			{
		//				&ConfigurationValue::Array(ref a) => project=Some(a.iter().map(|v|match v{
		//					&ConfigurationValue::True => true,
		//					&ConfigurationValue::False => false,
		//					_ => panic!("bad value in project"),
		//				}).collect()),
		//				_ => panic!("bad value for project"),
		//			}
		//			"legend_name" => (),
		//			_ => panic!("Nothing to do with field {} in CartesianTransform",name),
		//		}
		//	}
		//}
		//else
		//{
		//	panic!("Trying to create a CartesianTransform from a non-Object");
		//}
		match_object_panic!(arg.cv,"CartesianTransform",value,
			"sides" => match value
			{
				&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
					&ConfigurationValue::Number(f) => f as usize,
					_ => panic!("bad value in sides"),
				}).collect()),
				_ => panic!("bad value for sides"),
			}
			"shift" => match value
			{
				&ConfigurationValue::Array(ref a) => shift=Some(a.iter().map(|v|match v{
					&ConfigurationValue::Number(f) => f as usize,
					_ => panic!("bad value in shift"),
				}).collect()),
				_ => panic!("bad value for shift"),
			}
			"permute" => match value
			{
				&ConfigurationValue::Array(ref a) => permute=Some(a.iter().map(|v|match v{
					&ConfigurationValue::Number(f) => f as usize,
					_ => panic!("bad value in permute"),
				}).collect()),
				_ => panic!("bad value for permute"),
			}
			"complement" => match value
			{
				&ConfigurationValue::Array(ref a) => complement=Some(a.iter().map(|v|match v{
					&ConfigurationValue::True => true,
					&ConfigurationValue::False => false,
					_ => panic!("bad value in complement"),
				}).collect()),
				_ => panic!("bad value for complement"),
			}
			"project" => match value
			{
				&ConfigurationValue::Array(ref a) => project=Some(a.iter().map(|v|match v{
					&ConfigurationValue::True => true,
					&ConfigurationValue::False => false,
					_ => panic!("bad value in project"),
				}).collect()),
				_ => panic!("bad value for project"),
			}
			//"legend_name" => (),
			//_ => panic!("Nothing to do with field {} in CartesianTransform",name),
		);
		let sides=sides.expect("There were no sides");
		//let permute=permute.expect("There were no permute");
		//let complement=complement.expect("There were no complement");
		CartesianTransform{
			cartesian_data: CartesianData::new(&sides),
			shift,
			permute,
			complement,
			project,
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
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		for pattern in self.patterns.iter_mut()
		{
			pattern.initialize(source_size,target_size,topology,rng);
		}
	}
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
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
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Composition"
			{
				panic!("A Composition must be created from a `Composition` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"patterns" => match value
					{
						&ConfigurationValue::Array(ref l) => patterns=Some(l.iter().map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
						_ => panic!("bad value for patterns"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Composition",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Composition from a non-Object");
		}
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
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		self.pattern.initialize(source_size,target_size,topology,rng);
	}
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
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
		//if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		//{
		//	if cv_name!="Pow"
		//	{
		//		panic!("A Pow must be created from a `Pow` object not `{}`",cv_name);
		//	}
		//	for &(ref name,ref value) in cv_pairs
		//	{
		//		//match name.as_ref()
		//		match AsRef::<str>::as_ref(&name)
		//		{
		//			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
		//			"exponent" => match value
		//			{
		//				&ConfigurationValue::Number(x) => exponent=Some(x as usize),
		//				_ => panic!("bad value for exponent"),
		//			},
		//			"legend_name" => (),
		//			_ => panic!("Nothing to do with field {} in Pow",name),
		//		}
		//	}
		//}
		//else
		//{
		//	panic!("Trying to create a Pow from a non-Object");
		//}
		match_object_panic!(arg.cv,"Pow",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"exponent" => match value
				{
					&ConfigurationValue::Number(x) => exponent=Some(x as usize),
					_ => panic!("bad value for exponent"),
				},
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
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		self.target_size = target_size;
		if source_size!=self.cartesian_data.size
		{
			panic!("Sizes do not agree on CartesianFactor.");
		}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
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
		let mut sides=None;
		let mut factors=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="CartesianFactor"
			{
				panic!("A CartesianFactor must be created from a `CartesianFactor` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"sides" => match value
					{
						&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in sides"),
						}).collect()),
						_ => panic!("bad value for sides"),
					}
					"factors" => match value
					{
						&ConfigurationValue::Array(ref a) => factors=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f,
							_ => panic!("bad value in factors"),
						}).collect()),
						_ => panic!("bad value for factors"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in CartesianFactor",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a CartesianFactor from a non-Object");
		}
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
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		//XXX Do we want to check the user given destinations against target_size?
		for _ in 0..self.extra_random_destinations
		{
			let r=rng.borrow_mut().gen_range(0..target_size);
			self.destinations.push(r);
		}
		if self.destinations.is_empty()
		{
			panic!("The Hotspots pattern requires to have at least one destination.");
		}
	}
	fn get_destination(&self, _origin:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		let r = rng.borrow_mut().gen_range(0..self.destinations.len());
		self.destinations[r]
	}
}

impl Hotspots
{
	fn new(arg:PatternBuilderArgument) -> Hotspots
	{
		let mut destinations=None;
		let mut extra_random_destinations=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Hotspots"
			{
				panic!("A Hotspots must be created from a `Hotspots` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"destinations" => match value
					{
						&ConfigurationValue::Array(ref a) => destinations=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in destinations"),
						}).collect()),
						_ => panic!("bad value for destinations"),
					}
					"extra_random_destinations" => match value
					{
						&ConfigurationValue::Number(f) => extra_random_destinations=Some(f as usize),
						_ => panic!("bad value for extra_random_destinations ({:?})",value),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Hotspots",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Hotspots from a non-Object");
		}
		let destinations=destinations.unwrap_or_else(Vec::new);
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
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		if self.patterns.len()!=self.weights.len()
		{
			panic!("Number of patterns must match number of weights for the RandomMix meta-pattern.");
		}
		if self.patterns.len()==0
		{
			panic!("RandomMix requires at least one pattern (and 2 to be sensible).");
		}
		for pat in self.patterns.iter_mut()
		{
			pat.initialize(source_size,target_size,topology,rng);
		}
		self.total_weight=self.weights.iter().sum();
	}
	fn get_destination(&self, origin:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		let mut w = rng.borrow_mut().gen_range(0..self.total_weight);
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
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="RandomMix"
			{
				panic!("A RandomMix must be created from a `RandomMix` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"patterns" => match value
					{
						&ConfigurationValue::Array(ref a) => patterns=Some(a.iter().map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
						_ => panic!("bad value for patterns"),
					}
					"weights" => match value
					{
						&ConfigurationValue::Array(ref a) => weights=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in weights"),
						}).collect()),
						_ => panic!("bad value for weights"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomMix",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a RandomMix from a non-Object");
		}
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
	fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		self.size=target_size;
		self.pending=RefCell::new(Vec::with_capacity(self.size));
		//if source_size!=target_size
		//{
		//	unimplemented!("Different sizes are not yet implemented for GloballyShufflingDestinations");
		//}
	}
	fn get_destination(&self, _origin:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		let mut pending = self.pending.borrow_mut();
		if pending.is_empty()
		{
			for i in 0..self.size
			{
				pending.push(i);
			}
			//rng.borrow_mut().shuffle(&mut pending);//rand-0.4
			pending.shuffle(rng.borrow_mut().deref_mut());//rand-0.8
		}
		pending.pop().unwrap()
	}
}

impl GloballyShufflingDestinations
{
	fn new(arg:PatternBuilderArgument) -> GloballyShufflingDestinations
	{
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="GloballyShufflingDestinations"
			{
				panic!("A GloballyShufflingDestinations must be created from a `GloballyShufflingDestinations` object not `{}`",cv_name);
			}
			for &(ref name,ref _value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"legend_name" => (),
					//"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					//"servers" => match value
					//{
					//	&ConfigurationValue::Number(f) => servers=Some(f as usize),
					//	_ => panic!("bad value for servers"),
					//}
					//"load" => match value
					//{
					//	&ConfigurationValue::Number(f) => load=Some(f as f32),
					//	_ => panic!("bad value for load"),
					//}
					//"message_size" => (),
					_ => panic!("Nothing to do with field {} in GloballyShufflingDestinations",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a GloballyShufflingDestinations from a non-Object");
		}
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
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		self.size = target_size;
		let number_of_groups = (source_size+self.group_size-1) / self.group_size;// ts/gs rounded up
		self.pending=vec![RefCell::new(Vec::with_capacity(self.size)) ; number_of_groups];
		//if source_size!=target_size
		//{
		//	unimplemented!("Different sizes are not yet implemented for GroupShufflingDestinations");
		//}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		let group = origin / self.group_size;
		let mut pending = self.pending[group].borrow_mut();
		if pending.is_empty()
		{
			for i in 0..self.size
			{
				pending.push(i);
			}
			//rng.borrow_mut().shuffle(&mut pending);//rand-0.4
			pending.shuffle(rng.borrow_mut().deref_mut());//rand-0.8
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


#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformDistance
{
	///Distance to which destinations must chosen.
	distance: usize,
	///sources/destinations mapped to each router.
	concentration: usize,
	///`pool[i]` contains the routers at `distance` from the router `i`. 
	pool: Vec<Vec<usize>>,
}

impl Pattern for UniformDistance
{
	fn initialize(&mut self, source_size:usize, target_size:usize, topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		let n=topology.num_routers();
		//assert!(n==source_size && n==target_size,"The UniformDistance pattern needs source_size({})==target_size({})==num_routers({})",source_size,target_size,n);
		assert!(source_size==target_size,"The UniformDistance pattern needs source_size({})==target_size({})",source_size,target_size);
		assert!(source_size%n == 0,"The UniformDistance pattern needs the number of routers({}) to be a divisor of source_size({})",n,source_size);
		self.concentration = source_size/n;
		self.pool.reserve(n);
		for i in 0..n
		{
			let mut found: Vec<usize> = (0..n).filter(|&j|topology.distance(i,j)==self.distance).collect();
			found.shrink_to_fit();
			self.pool.push(found);
		}
	}
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)->usize
	{
		let pool = &self.pool[origin/self.concentration];
		let r=rng.borrow_mut().gen_range(0..pool.len());
		pool[r]*self.concentration + (origin%self.concentration)
	}
}

impl UniformDistance
{
	fn new(arg:PatternBuilderArgument) -> UniformDistance
	{
		let mut distance =  None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="UniformDistance"
			{
				panic!("A UniformDistance must be created from a `UniformDistance` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"distance" => match value
					{
						&ConfigurationValue::Number(f) => distance=Some(f as usize),
						_ => panic!("bad value for distance"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in UniformDistance",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a UniformDistance from a non-Object");
		}
		let distance = distance.expect("There were no distance");
		UniformDistance{
			distance,
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
}

impl Pattern for FixedRandom
{
	fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		self.map.reserve(source_size);
		let mut rng = rng.borrow_mut();
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
	fn get_destination(&self, origin:usize, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)->usize
	{
		self.map[origin]
	}
}

impl FixedRandom
{
	fn new(arg:PatternBuilderArgument) -> FixedRandom
	{
		let mut allow_self = false;
		match_object_panic!(arg.cv,"FixedRandom",value,
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
		FixedRandom{
			map: vec![],//to be intializated
			allow_self,
		}
	}
}

