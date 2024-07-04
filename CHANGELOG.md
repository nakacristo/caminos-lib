# Change Log

## next, [0.6.4]

Many things, including breaking changes...

### 2024-02-26
BUGFIX: Make the comparison `config_relaxed_cmp` used with the `--source` flag to verify arrays and alike to have the same length.

### 2024-02-14
Updated grammar to allow an array of expressions as expression.
Added `extra` argument to Plots outputs.

### 2024-01-22
Generate more parametrized symbol tex outputs, which are easier to manipulate from LaTeX side.

### 2024-01-19
Added a Debug pattern.
Put CartesianCut in quarantine.

### 2024-01-16
Fix journal message on shell action.
Added Switch pattern.

### 2024-01-15
Finished RandomLinkFaults.
Added default implementation for `Topology::{maximum_degree,minimum_degree}`, and algorithm `Topology::compute_diameter`.

### 2024-01-12
Added `ConfigurationValue::as_rng`.
Added optional seed to RandomPermutation.
Added meta-topology RandomLinkFaults.

### 2023-12-21
Tuned usage of tikzexternalize to behave reasonably when using `-no-shell-escape`.

### 2023-12-05
Sanitize some generated latex comment.
Added routing AdaptiveStart.

## [0.6.3]

### 2023-12-01
Include fix of TrafficMap.
Publish 0.6.3.

### 2023-11-29
Some tikz improvements.
Removed the restriction on results count for plotting.
Added support for selecting targets to output.
Merged several things from Alex.

### 2023-11-27
Some documentation improvements.

### 2023-11-15
Only check for equality of tasks and servers at Simulation level, i.e., the traffic at the configuration root.

### 2023-11-07
New traffic [TrafficMap].
BREAKING CHANGE: Added method `number_tasks`required for trait Traffic.
BREAKING CHANGE: Renamed in Traffic nomenclature servers into tasks. This includes ServerTrafficState renamed into TaskTrafficState, and `server_state` into `task_state`. Old configuration names are still supported.
Allow [Composition] of patterns with different sizes at middle points.

### 2023-11-03
Added links to `new_traffic` documentation and related improvements.

### 2023-11-02
New pattern [CartesianCut].
Added `PatternBuilderArgument::with_cv`.

### 2023-10-30
Improved documentation of `new_topology`.
New pattern [RemappedNodes].

### 2023-10-26
Added multiplier to CartesianTransform.
Added links in pattern documentation.
New pattern [CartesianEmbedding].

### 2023-10-25
Merge with alex-master.
Added documentation for the link classes employed in Dragonfly.
Updated `divisor_frequency` in default configuration file.

### 2023-10-19
When building PDF, in addition to `git_id`, show also the version number.

## [0.6.2]

### 2023-10-13
Publish 0.6.2.

### 2023-10-09
Some output tweaks.
Added [AsCartesianTopology] meta-topology to gives sides to an arbitrary topology.

### 2023-10-03
Manage separately the path as passed by terminal as the path detected by `std::env::current_exe`.

### 2023-10-02
Add `sum` as alias for the `add` config function.

### 2023-09-25
`canonicalize` the path to CAMINOS' binary in ExperimentFiles, to be apt for SLURM scripts.

### 2023-09-18
Fix InputOutput router to allow for `crossbar_delay=0`.
Little cleaning on SumRouting.

### 2023-09-13
Fixes and cleanup on the InputOutput router.
BREAKING CHANGE: Router methods insert and acknowledge now return `Vec<EventGeneration>` and are responsible for their scheduling.
Avoid unnecessary scheduling of the InputOutput exits.

### 2023-09-12
BREAKING CHANGE: All cycles are now represented by a `Time` alias of `u64`; instead of `usize`.
BREAKING CHANGE: Removed methods `pending_events`, `add_pending_event`, and `clear_pending_events` from the Eventful trait in favor of the `schedule` method.
Added TryLinkTraversal to InputOutput router to decouple the scheduling of the crossbar and the output links.

### 2023-09-11
Allow different frequencies in links and the InputOuput router.
The tentative configuration parameter `transference_speed` in links have been removed in favor of `frequency_divisor`, now functional.

### 2023-09-08
More work on frequencies.

### 2023-09-07
Adding ways to divide the frequency of components.

### 2023-07-25
New action QuickTest.

### 2023-07-17
New error IncompatibleConfigurations, FileSystemError, RemoteFileSystemError.

## [0.6.1]

### 2023-07-13
Update to gramatica-0.2.1, that has updated its regex to 1.9.1, notably improving performance.
Publish as 0.6.1.

### 2023-07-12
Added tests for flattening configurations.
Allow flattening for more recursive combinations of Experiments and NamedExperiments.
Configuration output in terminal more compact and legible.

### 2023-07-07
Updated some 0.6 in the README.
Merged field `random` for CartesianTransform.
New argument patterns for CartesianTransform.
New pattern Circulant.

## [0.6.0]

### 2023-06-29
New router option `neglect_busy_output`.
git tag 0.6.0 -m "v0.6.0"
Publish as 0.6. 

### 2023-06-27
Merged some config functions from Alex.
Added some style to the configuration in latex output.

### 2023-06-26
Added method `ConfigurationValue::format_terminal` for better formatted output.
Add format to the configuration shown in latex output.
BREAKING CHANGE: Routers now set `router_allows` to false instead of just discarding the candidates to queues already in use.

### 2023-06-23
More options for UpDownStar.
Added `value.as_i32()`.

### 2023-06-20
Improve the content of some message.

### 2023-06-19
In UpDownStar, restricted cross branching to horizontal links.

### 2023-06-14
Added a test to UpDownStar.

### 2023-06-07
Added cross-branch option to ExplicitUpDown.

### 2023-05-26
Rename `input_output_monocycle.rs` as just `input_output.rs`. It was never exactly a monocycle.
And the router called `InputOutput`.
Added `allow_self` to Uniform. Also removed unnecessary loop.

### 2023-05-10
Added a `sync` call to the slurm jobs to help them start with an up to date filesystem.

### 2023-05-04
Fixed sbatch arguments.

### 2023-05-03
Added `intermediate_bypass` to Valiant routing.
Added `sbatch_args` so the slurm options to pass an arbitrary list of strings.
Changed interface of `Routing` to contain the target router and making the target server optional. Also, `next` returns an `Error`.

### 2023-04-29
Update some tests that were still using `RefCell<StdRng<_>>`.
Added a test for the sort config function.
Improved to sort config function to clone the context only once, instead of twice per comparison.

### 2023-04-28
BUGFIX: bad contexts were used in the sort config function.

### 2023-04-03
Changed latex backed to use utf8.

### 2023-03-29
Added SimulationShared and SimulationMut for better encapsulation.
Replaced every `&RefCell<StdRng>` by `&mut StdRng` everywhere.
Added a prelude to the router module.

### 2023-03-23
Fix checks in InputOutputMonocycle relative to the crossbar delay.

### 2023-03-22
Partial implementation of `InputOutputMonocycle::crossbar_delay`.
Added configuration option `crossbar_delay` to InputOutputMonocycle with a default value of 0.

### 2023-03-20
Made `Basic::time_at_input_head` work again, hopefuly.

### 2023-03-14
Added documentation for FileMap.

### 2023-03-10
Finished Dragonfly2ColorsRouting.

### 2023-03-09
Added a prelude to routing.
Added Dragonfly2ColorsRouting.
Topologies now may expose a `dragonfly_size`.
Allow to vary `number_of_groups` in dragonfly topology.

### 2023-03-08
CanonicDragonfly renamed as just Dragonfly.
Allow aliases in macros `match_object` and `match_object_panic`.

### 2023-03-02
Added `ConfigurationValue::as_usize`.
New policy VOQ.

### 2023-03-01
Updated the UniformDistance and RestrictedMiddleUniform to be sensible on indirect networks.

### 2023-02-28
Added kernel info to memory report.
Added Quantifiable to more types.

## [0.5.4]

### 2023-02-24
Merged a little from alex into multistage.
Added else clause to RestrictedMiddleUniform.
Fixed plot-point-eating bug.
Added configuration option `memory_report_period`.
Fixed vc of injection.
Publish 0.5.4.

### 2023-02-23
New pattern RestrictedMiddleUniform.

## [0.5.3]

### 2023-02-21
Publish 0.5.3.

### 2023-02-20
Merge some things from alex branch, with minor changes.
Default `Topology::coordinated_routing_record` to unimplemented.
Default `Topology::is_direction_change` to false.
Added a prelude submodule to pattern.
New meta-topology RemappedServersTopology.

### 2023-02-17
Comment out the `strong_link_classes` that was added to Polarized, as resulted not being useful.

### 2023-02-15
Documentation typo.

### 2023-02-14
Fixed a test.
Added `strong_link_classes` field and option to Polarized routing.

### 2023-02-08
Added a method `Arrangement::initialize`.
Added `global_arrangement` configuration option to both Dragonfly and Megafly.
implemented Random arrangement for Dragonfly and Megafly.

### 2023-02-07
Added some iterators to Matrix.
Adding global arrangements to Dragonfly and Dragonfly+.
Megafly is now operative.

### 2023-02-06
Added Quantifiable to StdRng to ease adding local seeds.
Work on the Dragonfly+ (Megafly).
Some improvements on Dragonfly documentation.

### 2023-02-03
Fix the alternative of `relative_sizes` in the pattern IndependentRegions.
Auxiliar function `pattern::proportional_vec_with_sum`.
Added file megafly.rs.

### 2023-01-31
Documentation fix.
New meta-pattern IndependentRegions.

### 2023-01-28
Documentation fix.

## [0.5.2]

### 2023-01-26
Publish 0.5.2.

### 2023-01-23
Improved the test of FixedRandom.
Added a test in the config module.

### 2023-01-19
Added some test for FixedRandom.

### 2023-01-17
Made the same changes on InputOutputMonocycle.
Use `match_object_panic`to build the routers Basic and InputOutputMonocycle.
Renamed TransmissionFromServer into TransmissionFromOblivious.

### 2023-01-10
Servers now select a virtual channel to send a packet to the router.
Basic router server-router mechanism now defaults to SimpleVirtualChannels.

### 2023-01-09
Some rework on TransmissionMechanism, including removing the generic parameter from `Router::Basic<TM>`.
Allow servers to have different StatusAtEmissor.
Added method `Router::build_emissor_status` to build the status at the servers.
Enhanced Basic router to select different transmission mechanism.

### 2022-11-18
Correct `servers_with_missed_generations` in documentation.

### 2022-11-17
Added pow config function.

### 2022-11-04
Added the Polarized routing to the public repository.
Added routing documentation.

### 2022-09-30
Added naive implementation of `MultiStage::{minimum_degree,maximum_degree}`.

### 2022-09-23
Try on slab crate.

### 2022-09-16
New file packet.rs, and moved into it the Phit, Packet, and Message structs.
Implemented feature `raw_packet`, to use raw pointers instead of `Rc<Packet>`.

## [0.5.1]

### 2022-09-16
Minor documentation improvements.
Tag as 0.5.1.

### 2022-09-08
Updated default main.od to include human values for memory and time.

### 2022-09-07
Added license files.
Brought allocator, the so called BasicModular router and a couple related things from Daniel Postigo located at https://github.com/codefan-byte/caminos-tfg/tree/router-allocator-daniel/src/allocator .
Renamed the PortRequest from `basic_modular` as VCARequest and moved it into the allocator mod.
Changed label for AVCRequest on missing priority from -1 to 0, to comply with the statet convention; althought is it unused.
New BasicModular routing renamed onto InputOutputMonocycle.
Renamed file `basic_modular.rs` into `input_output_monocycle.rs`.
Rename IslipAllocator into ISLIPAllocator, and make some configuration aliases.

## [0.5]

### 2022-09-07
git tag 0.5.0 -m "v0.5.0"
Publish as 0.5. 

### 2022-07-11
Added `RoutingInfo::auxiliar` to allow using arbitrary types.
git commit -m 'Allow arbitrary data in RoutingInfo. Improvements to tikz backend. New `try` config function.'

### 2022-07-07
Prevent some errors from panic in `config::evaluate`.
Added config function `try`.

### 2022-06-28
Avoid scaling ticks when using the time/memory tick styles.

### 2022-06-27
Show less significant figures with `timetickcode` within the tikz backend.
Added tick styles `{x,y} memory ticks from kilobytes`.
Minor improvements to the generated latex code.

### 2022-06-23
Addedd tikz styles `{x,y} time ticks`.

### 2022-06-09
FIX Up*/Down* routing, which was wrong in multiple ways.
git commit -m "Fixed UpDownStar."

### 2022-06-08
Comment out `dbg!` statements in `ExplicitUpDown`.

### 2022-06-07
Described some error.
Do not merge Nones from external experiments.

### 2022-06-05
git commit -m "Fix statistics of nested SumRouting."

### 2022-06-04
More fixes on statistics of SumRouting.

### 2022-06-03
Added default implementations of some methods of Routing.
Added statistics to SumRouting.
git commit -m "Added statistics to SumRouting. Added default implementations to some methods of the Routing trait."
Some fixes on SumRouting.

### 2022-06-02
`Error::with_message` now appends the message is some already exists.
`OutputEnvironment::map` now is and uses functions returning Result.
Now errors in `OutputEnvironment` and other places are propagated through their closures.
git commit -m "Propragate some errors."

### 2022-05-31
More sane CSV output. Allow to set the header names.
Added `--foreign` and `--use_csv` terminal options.
BREAKING CHANGE: `config::{evaluate,reevaluate}` now returns a `Result`.
git commit -m "Added capability with working with foreign CSV data. Updated the API of evaluate and reevaluate to return a Result."

### 2022-05-30
Add some `ignore` in the documentation to avoid `cargo test` errors.
git commit -m "Ignore all doc errors."
Made `terminal_default_options` and `terminal_main_normal_opts` from code previously on the binary crate.
Moved `special_export` from the binary crate.
git commit -m "Moved some code from the binary into the library."

### 2022-05-24
git commit -m "Be able to generate boxplots without average marks."

### 2022-05-17
Method `Action::from_str` now returns a better error.
New action `Discard`.
Added dependency on `rprompt` crate.
Renamed KeyboardInteration [sic] into KeyboardInteraction.
Ask for permission when deleting result files.
git commit -m "New action Discard."
git commit -m "Added option --interactive=bool to control whether to ask for confirmation."

### 2022-05-16
Avoid repeatedly cloning of contextes in the `map` and `filter` config functions.
Tried Addind BorrowedConfigurationValue to ease up copy-free management of ConfigurationValues. But it has proved too tricky.
git commit -m "Avoid multiples calls to clone on a context in map and filter config function. Added a commented-out code that could help with these situations, see BorrowedConfigurationValue. But it is tricky."

### 2022-05-11
Added variant `EmbeddedMap` of `FileMap` with the data directly on the .cfg.
Added new pattern CartesianTiling.
git commit -m "New patterns EmbeddedMap and CartesianTiling."

### 2022-05-10
Added support for more authentication methods, `publickey` and `keyboard-interactive`, in addition to the already used `password` method.
For the publickey method behave like ssh: By default try all private keys. If there is some user configuration then use those.
git commit -m "Implemented SSH authentication methods publickey and keyboard-interactive."

### 2022-05-03
Added `ServerMeasurement` to manage the statatistics of servers in a similar way to the global ones.
Methods `jain_server_consumed_phits` and `jain_server_created_phits` moved to `Network`.
Added a `measures::jain` method.
Added temporal statistics to the servers.
git commit -m "Added temporal statistics to the servers and other improvements."

### 2022-04-29
New struct `output::PlotData` to better encapsulate processed data.
Updated OutputEnvironment.
Make sftp build also parent directories when they are missing.

### 2022-04-28
git commit -m "Another fix on removing the single request requirement."

### 2022-04-27
Many more advices taken from clippy.
git commit -m "New abstraction OutputEnvironment. New file clippy.toml. Implemented changes from clippy."
BREAKING CHANGE: Using `&dyn Topology` instead of `&Box<dyn Topology>` in all interfaces.
git commit -m "Removed references to boxes in interfaces."
BREAKING CHANGE: `Topology::coordinated_routing_record` now receives slices.
git commit -m "coordinated_routing_record now receives slices."
BREAKING CHANGE: `CartesianData::new` now receives an slice.
git commit -m "CartesianData::new now receives an slice."
Upgraded the Result types in projective.rs and multistage.rs to use our Error type.
BREAKING CHANGE: SpaceAtReceptor and Stage now use the Error type in its Result types.
git commit -m "SpaceAtReceptor and Stage now use the Error type in its Result types."
Update default main.cfg to use the correct name `output_prioritize_lowest_label`.
git commit -m "Trying removing the single request per input buffer requirement."
Fix (done of total) label in the generated latex.
git commit -m "More on removing the single request requirement."

### 2022-04-26
Taken some advices from clippy.
Added a clippy.toml to keep configuration of the clippy assistant.
Added some crate attributes to control messages of clippy.

### 2022-04-13
Use correct English for field `router::Basic::output_prioritize_lowest_label`. The old config name `output_priorize_lowest_label` is deprecated but accepted with a warning.
git commit -m "Use correct english for output_prioritize_lowest_label, warn when misused."
Created a new wrapper `output::OutputEnvironment` to separate a bit the output generation from the experiment actions.
The output handlers now work with iterators of contexes, instead of creating them.
Implemented Debug for ExperimentFiles.

### 2022-04-11
BUGFIX: The macros `match_object_*` had a hard-coded Pow.
git commit -m "Fixed an error message from the match_object macros."

### 2022-04-08
Mention `texlive-latexextra` in the documentation.
Split file measures.rs from lib.rs.
git commit -m "New file measures.rs, split from lib.rs."

### 2022-04-07
Fix the tracking of the recently added `virtual_channel_usage`.
Fixes to regenerate the legends in the tikz backend.
Allow None in some config functions.
Added new policy operation `Chain` to use inside meta-policies.
git commit -m "Policy Chain and a variety of fixes."

### 2022-04-06
Added `Statistics::current_temporal_measurement` to replace the shared code for temporal statistics in the several `track*` methods.
Added tracking link usage by virtual channels. Exposed results by the name `virtual_channel_usage`.
git commit -m "Added statistic virtual_channel_usage. Removed the weird legend-self-references generated from the tikz backend."
Tikz do no calculate dependencies unless using `/tikz/external/mode=list and make`, but that requires calling a Makefile.
Instead of all this shit now I call the legend subjobs directly.
See the documentation of `\tikzpicturedependsonfile` saying
> Limitations: this command is currently only supported for mode=list and make and the generated makefile.

### 2022-04-05
Changed in tikz backed from using ref to using pgfplotslegendfromname.

### 2022-04-01
git commit -m "Upgraded constructors in lib.rs to use the config helpers."
git commit -m "Upgraded the SlurmOptions constructor in experiment.rs"
Fix on macro `match_object!`.
git commit -m "Upgraded output.rs to use the config helpers."
git commit -m "Upgraded the traffic constructors to use the config helpers."

### 2022-03-31
BUGFIX: Protect the building of temporal statistics from routers having different measurement arrays of different lengths.
git commit -m "several small fixes."
git commit -m "More helpers in config.rs and all pattern constructors updated to use them."
git commit -m "Updated routing constructors to use the config helpers."
git commit -m "Updated policies constructors to use the config helpers."
git commit -m "Moved routing into its own folder to break it later into multiple files"
git commit -m "Split the routing mod into five files: mod, basic, extra, channel_operations, updown."
git commit -m "More fix to the temporal statistics of the Basic router."

### 2022-03-30
`TimeSequenced::should_generate` now returns false instead of panicking when the traffic index gets over the limit.
git commit -m "fixed TimeSequenced stop."
Added reasons for the dependencies.
Upgrade ssh2-0.8.2 to ssh2-0.9. No changes made in code.
Upgrade rpassword-5.0 to rpassword-6.0. A little rewrite required.
Upgrade indicatif-0.15.0 to indicatif-0.16. Removed an ampersand.
Upgrade procfs-0.9 to procfs-0.12. No changes made in code.
Removed dependency on lazy_static.
git commit -m "Commit after upgrading all dependencies to date."
Little fix on the `error!` macro.
Added function `as_bool` to ConfigurationValue.
Use `StdRng::seed_from_u64` instead of creating a `[u8;32]` for `::seed`.

### 2022-03-29
git commit -m "error and match_object macros. With the action shell and source the remote folder name is rewritten."
git commit -m "Upgraded dependency rand-0.4 to rand-0.8. It has been more tricky than expected."
git commit -m "fixed some comments."

### 2022-03-26
Minor improvements.

### 2022-03-25
git commit -m "Added temporal statistics to the Basic router."
In the action shell, when giving a source, rewrite the source folder name into the new folder name.
Added an `error` macro for the sake of writting error management.
Added macro `match_object` in the config module to ease up unpacking values.

### 2022-03-24
New field `RouterBuilderArgument::statistics_temporal_step` to inform about the corresponding field in `Statistics`.
Added to basic router methods `gather_cycle_statistics` and `get_current_temporal_measurement`, and associated fields.
Updated constructor of `Basic` router to use `RouterBuilderArgument`.
The method `aggregate_statistics` in the basic router now includes temporal statistics.

### 2022-03-07
Small documentation fix on TimeSequenced traffic.
Implemented `config_relaxed_cmp` to ignore some small differences in experiment configurations.
Allow to merge experiments even if the `legend_name` and `launch_configurations` do not match.
git commit -m "Use a relaxed comparison of configs while merging results with --source"

### 2022-03-02
Documented some patterns.
Added pattern FixedRandom.
Added optional field `project` to pattern `CartesianTransform`.
git commit -m "New pattern FixedRandom and field project in CartesianTransform."

### 2022-03-01
git commit -m "Removed panic when executing the shell action. Create default files other than main.cfg if they are not in the given source."
git commit -m "Update the progress bar message when finishing it."
BREAKING CHANGE: Added phit to `RequestInfo`.
New policy `MapMessageSize`.
git commit -m "Added a new meta-policy, MapMessageSize. RequestInfo now contains the phit."

### 2022-02-28
Server queue size made a configuration option. `Simulation::server_queue_size`.
git commit -m "Added option server_queue_size."

### 2022-02-25
BREAKING CHANGE: Added a server argument to `Traffic::try_consume`.
git commit -m "New traffic BoundedDifference."
Added config function `sub` and aliases `plus`, `minus`.
git commit -m "config fucntion sub and aliases"

### 2022-02-23
Wrapped the progress bar into a new struct.
git commit -m "Added a statistic about missed message generations. Some improvements on error management and visualization."
More minor improvements on display of errors and others.
Slurm options moved into its own structs. And added a `wrapper` option to interpose some script into the slurm scripts.
git commit -m "Added a wrapper option for slurm and other minor improvements."

### 2022-02-22
Added field `ServerStatistics::missed_generations`. And added several derivated metrics to the results.
Write some error to stderr instead of stdout.
Converted some panic into `Err`.

### 2022-02-18
BUGFIX: Get the `buffer_size` when building the servers and minor fixes on TransmissionFromServer.
git commit -m "Fix in server initialization and improvements on error management."

### 2022-02-17
Skip output generation if there are too few results.
Created a new module `error.rs`.
Added a bit of error management in a couple of functions. Many more to do later.
Added a couple documentation phrases to the `experiments` module.
Removed the `panic:bool` flag `build_cfg_contents` as it is controled now via `Result`.

### 2022-02-16
The pattern `ConstantShuffle` has been renamed into `GloballyShufflingDestinations` and improved a bit.
Added a similar pattern `GroupShufflingDestinations`, to have something closer to its original idea.
Added `ServerTrafficState::FinishedGenerating`, since it represent the "finished" state more quickly reached and easily determined.
Added `Pattern::Identity` for ease of meta-patterns.
git commit -m "more patterns"
New traffic `MultimodalBurst`.
Added `Quantifiable` to more tuples.
git commit -m "added traffic MultiModalBurst"
git commit -m "fix on MultimodalBurst"
git commit -m "do not panic when the slurm error file does not exist"
git commit -m "write stderr_file correctly"

## [0.4.3]

### 2022-02-15
Added `Pattern::UniformDistance`.
git commit -m "added UniformDistance pattern"
Defined a function to query the maximum number of jobs allowed on the slurm system.
Bring version number in another way.
Fixed UniformDistance pattern.
git commit -m "query slurm jobs, fix UniformDistance, fix version number"
git commit -m "really fix UniformDistance"
git commit -m "again with UniformDistance..."
git tag 0.4.3 -m "v0.4.3"
git commit -m "publish 0.4.3"

### 2022-02-14
Removed verbose message when gnerating outputs.
Do not panic when remote main.cfg does not exist while pushing.


## [0.4.2]

### 2022-02-14
Use `latex_protect_text` instead of `latex_make_symbol` for labels from non-numerical values.
BUGFIX: made `--action=remote_check` to work again. Also added display for the remote stderr.
BUGFIX: allow push action to work if remote directory exists but main.cfg does not.
Declaration of Module `router::Basic` made public, so that its documentation is generated.
Added `get_version_number`, in parallel to `get_git_id`.
git commit -m "Get version number. Fixes with latex and actions on remote."
BREAKING CHANGE: functions on the output module now use ExperimentFiles instead of Path.
Generated outputs moved into their own directory.
Added field `Experiment::experiment_to_slurm` to tie experiment numbers to launcher scripts.
Added capability to check errors in the launch script standard error output.
git commit -m "Outputs moved to their own directory. Check error files."
git tag 0.4.2 -m "v0.4.2"
git commit -m "publish 0.4.2"

### 2022-02-12
Fix on the tikz backend `/tikz/.cd`.

### 2022-02-11
Writing also average values from `cycle_last_created_phit` and `cycle_last_consumed_message`.
git commit -m "Couple minor bugfixes plus some time of last phit statistics."

### 2022-02-10
Added the function `server_state` to the `Traffic` trait.
BUGFIX in Shifted,Product traffic probability.
Added `ServerStatistics::{cycle_last_created_phit: usize,cycle_last_consumed_message: usize}`.

## [0.4.1]

### 2022-02-10
git commit -m "Removed spurious error when looking for launch_configurations"
git commit -m "Expose consumed_cycle to percentile statistics."
git tag 0.4.1 -m "v0.4.1"
git commit -m "publish 0.4.1 because of the slurm launch error."

## [0.4]

### 2022-02-10
git tag 0.4.0 -m "v0.4.0"
git commit -m "into 0.4.0. Added command line overrides and generation of default files. A statistics bugfix."
git commit -m "Have Cargo.toml also on 0.4.0"
git commit -m "Made the doc breaking changes section into a dropdown."
git commit -m "Documented shell and pack actions."

### 2022-02-09
Moving more things into struct ExperimentFiles.
BUGFIX: Some delays were added into `total_message_delay` instead of into `total_packet_network_delay`.
Added default experiments files main.cfg, main.od, remote that are generated with --action=shell and no --source.
A couple more progress bar display prefixes.

### 2022-02-08
Added `rewrite_eq` and `rewrite_pair` to allow writing into a ConfigurationValue. With the idea of using the free arguments in command line.
New struct ExperimentFiles to encapsulate better the files in different places.

### 2022-02-02
Added the MapEntryVC meta-policy to build rules dependant on the virtual channel with which the packet entered the router.
git commit -m "MapEntryVC policy, div config function, and ordinate_post_expression field."
git commit -m "Updated gitignore"

### 2022-01-31
Added div config function.
Added `ordinate_post_expression`to `Plotkind`.
Changed sbatch job name to CAMINOS.

### 2022-01-12
New pattern ConstantShuffle.
git commit -m "New pattern ConstantShuffle."

### 2021-12-16
New meta routing option `SumRoutingPolicy::EscapeToSecond`.
New `VirtualChannelPolicy::{ArgumentVC,Either}`.
git commit -m "Added an escape policy"
git commit -m "Added the Either channel policy to keep candidates satisfying any of several policies."

### 2021-12-09
Read remote binary.results when initializing remote.
Pull now try first to pull from binary.results.
Added config functions `slice`, `sort`, `last`, `number_or`, and `filter`.
Stop using tikz symbolic coordinates and use instead just natural coordinates with textual labels.
Improved the code to manage the plots.
Plots requiring symbols can now use absicssa limits.
git commit -m "Added several config functions and latex output improvements."
git commit -m "Added Diplay for FunctionCall expressions."

### 2021-12-07
Avoid making the runx directories when they are not required.
Added action `Pack`, to pack current results into binary.results and delete the raw ones.
git commit -m "New action pack"
git commit -m "moved a canonicalize out of the main path to avoid requiring the runx directories."
git commit -m "Added a canonicalize to the parent runs path"
git commit -m "bugfix on packet statistics: only track the leading phit of packets."
Some fixes to detect non-numbers before averaging.
Added `latex_make_symbol` to protect symbolic coordinates.

### 2021-12-04
Added `PacketExtraInfo` to `Packet` to store additional statistics for `statistics_packet_definitions`.
git commit -m "Added to statistics_packet_definitions the members link_classes, entry_virtual_channels, and cycle_per_hop"
git commit -m "The stat entry_virtual_channels now sets None value when a VC was not forced, as from the server"
git commit -m "Changed NONE VALUE to None"

### 2021-12-03
Removed an underflow when averaging consumption queues of the server in the Basic router.
New policy `MapHop` that applies a different policy to each hop number.
git commit -m "Added MapHop policy and diff"
Added user definied statistics for consumed packets. Define with `configuration.statistics_packet_definitions` and receive into`result.packet_defined_statistics`.

### 2021-12-01
git commit -m "fixed MapLabel: above and below were swapped in filter."

### 2021-12-01
Added crate `diff` to the dependencies.
Show differences on the configurations when there are any with the remote file.

### 2021-11-30
Added config functions `map` and `log`.

### 2021-11-29
git commit -m "relaxed Topology::check_adjacency_consistency for non-regular topologies."
git commit -m "Implemented distance method for Mesh topology."

### 2021-11-26
Fixed entry `ShiftEntryVC` on `new_virtual_channel_policy`.
git commit -m "fixed entry on new_virtual_channel_policy"
git commit -m "Added information on new_virtual_channel_policy panic"

### 2021-11-25
One point is enough is bar/boxplot graphs to consider them good plots.
Added `VirtualChannelPolicy::{Identity,MapLabel,ShiftEntryVC}`.
Breaking change: Added requirement `VirtualChannelPolicy: Debug`.
git commit -m "Policies are now required to implement Debug. New policies Identity, MapLabel and ShiftEntryVC."

### 2021-11-22
git commit -m "return from routings changed to RoutingNextCandidates and added idempotence checks."

### 2021-11-18
Refer to `texlive-pictures` in the README.md.
Adding `Action::Shell`.
Added documentation to `output.rs` and made it public to actually have docs to be generated.
git commit -m "boxplots, preprocsessing output files, improvements on documentations, shell action, and more."
Breaking change: routings now return `RoutingNextCandidates`. In addition to the vector of candidates it contains an `idempotent` field to allow some checks and optimizations.
Basic router now check idempotence of the routing to panic when there are no candidates for some packet.

### 2021-11-17
Added `Sequence` traffic.
New policy `SumRoutingPolicy::SecondWhenFirstEmpty` to complete a routing with another when the first does not find any candidates.

### 2021-11-10
Made `preprocessArgMax` work with incomplete data.
Fixed a bit the documentation syntax.

### 2021-11-01
Added preprocessing outputs: `PreprocessArgMax`.
New config functions `mul` and `FileExpression`.
Added `path` argument to `config::{evaluate,reevaluate}`.
File `create_output` and similar now receive in its `results` argument also the experiment indices.

### 2021-10-28
Improved style of Box Plots.

### 2021-10-28
Added option to generate Box Plots.

### 2021-10-27
Added `Statistics.server_percentiles` and configuration `statistics_server_percentiles` to generate in the result file fields such as `server_percentile25` with values of the server in the given percentile.
git commit -m "Added statistics_server_percentiles"
Added `Statistics.{packet_percentiles,packet_statistics}`, struct StatisticPacketMeasurement and configuration `statistics_packet_percentiles` to generate per packet statistics percentile data.
git commit -m "Added statistics_packet_percentiles"
Protect some latex labels.

## [0.3.1]

### 2021-10-19
Updated readme to say 0.3 and `pgfplots`.
Canonicalize path before extracting folder name, to work when a dot is given as path.
Cargo.toml version to 0.3.1.
git tag 0.3.1 -m "v0.3.1"
git commit -m "version update to 0.3.1, fixing using dot as path."

## [0.3.0]

### 2021-10-19
Fixed example configuration in the readme.
git commit -m "Support for bar graphs. meta routing EachLengthSourceAdaptiveRouting. readme fixes."
git tag 0.3.0 -m "v0.3.0"
git commit -m "version update to 0.3.0"
git commit -m "update to 0.3.0 in Cargo.toml"

### 2021-09-17
Implemented `EachLengthSourceAdaptiveRouting` as source routing storing a path of each length.

### 2021-07-16
Added styles for bars.

### 2021-07-15
Updated merge functionality to work with binary results.
Added `eq | equal` config evaluation function.
Added support for symbolic abscissa.

### 2021-07-07
git commit -m "Added hop estimation to Shortest and Valiant candidates."

### 2021-07-05
Set `estimated_remaining_hops` in `SourceRouting`.
Added `use_estimation` to `LowestSinghWeight`.
git commit -m "Generate hop estimations in SourceRouting and use them in LowestSinghWeight"
git commit -m "Enhancing LowestSinghWeight with things in OccupancyFunction"
New pseudo-routing wrapper `SourceAdaptiveRouting`.
git commit -m "New wrapper for source adaptive routings."

### 2021-05-21
git commit -m "fixes on using binary results"

### 2021-05-20
Added `already` count to progress bar message.
Fixed detection of results in binary format.

### 2021-05-18
Read results from binary.results.
Changed in `config_from_binary` things from `usize` to `u32` to clear sizes in binary format.
Pull remote results into memory and then into binary.results, instead of copying the local files.
git commit -m "Pack results into a bianry file"

### 2021-05-12
Implemented `config_to_binary`, `config_from_binary`, and BinaryConfigWriter. Tested to create them, remains to test loading them.

### 2021-05-10
Added field `Packet::cycle_into_network` to allow some additional statistics.
Removed `track_packet_hops` and added functionality to `track_consumed_packet`.
Added `average_packet_network_delay` to statistics at several levels.
git commit -m "Added network delay statistics per packet."

### 2021-05-08
SumRouting attributes converted into arrays to allow indexing.
Split SumRouting policy `TryBoth` into `TryBoth`, `Stubborn`, and `StubbornWhenSecond`.
git commit -m "stubborn policies on SumRouting."

### 2021-05-07
Space marks with tikz backend only when there are many points in drawing range.

### 2021-05-05
git commit -m "Added initialize recursion to Stubborn routing."

### 2021-05-04
git commit -m "Added performed_request recursion to Stubborn routing."

### 2021-05-03
git commit -m "Fixed a bug on the allowed virtual channels in SumRouting."

### 2021-04-30
Added `{min,max}_abscissa` to Plotkind.
Make AverageBins return NANs instead of panicing.
Automatically add `mark repeat` when having too many points within the tikz backend.
Fixed tracking temporal stastistics of given hops and message delay.
git commit -m "Fixes and improvemets for temporal statistics."

### 2021-04-23
git commit -m "Bugfix on WeighedShortest. New routing transformations related to virtual channels."
Removed `non_exhaustive` for Builders.

### 2021-04-22
Added routing `ChannelsPerHopPerLinkClass` and `AscendantChannelsWithLinkClass` and `ChannelMap`.
Routing `WeighedShortest` made to verify the selected link actually belong to the shortest route.
Implemented nesting of `Valiant` routing initialization.

### 2021-04-20
Added routing `ChannelsPerHop`.
git commit -m "Updated grammar tech to manage large files. New routing ChannelsPerHop."

### 2021-04-16
Removed grammar warning.
Use public gramatica-0.2.0.

### 2021-04-15
Updates in grammar technology.
Added `ConfigurationValue::None` to be able to implement `Default` and use `std::mem::take`.

### 2021-04-08
Messing with the grammar to avoid cloning values.
New configuration function `AverageBins`.

### 2021-04-07
Trying experimental gramatica-0.1.6 to solve the stack overflow.

### 2021-03-30
Debugging a stack overflow...

### 2021-03-29
Changed default statistic jain column to ServerGenerationJainIndex.
New traffic TimeSequenced.
Added parameter `cycle` to `Traffic::should_generate`.
Split StatisticMeasurement from the Statistics struct.
Added support to temporal statistics via `statistics_temporal_step`.
git commit -m "Improvements on Valiant routing, matrices, traffics, and statistics. Implemented optional tracking of statistics per cycle."

### 2021-03-26
Documentation improvements.
Derive Debug for RequestInfo.

### 2021-03-23
Documentation fix.

### 2021-03-22
Starting with ExplicitUpDown: implemented UpDownStar construct.
Added methods `Matrix::{get_rows,get_columns}`.

### 2021-03-18
Added to Valiant routing the optional parameters `{first,second}_reserved_virtual_channels: Vec<usize>` to easy defining a Valiant over MultiStage topologies using UpDown first with some virtual channel and later with other.

### 2021-03-18
Removed some `dbg!` statements from MultiStage.
git commit -m "Removed some debug statements."
Added optional parameter `selection_exclude_indirect_routers` to Valiant routing.
Added warning message when generating traffic over a different amount of servers than the whole of the topology.

### 2021-03-15
Converting `MultiStage::up_down_distances` from `Vec<Vec<Option<(usize,usize)>>>` into `Matrix<Option<(u8,u8)>>`.
Added `Matrix::map` to ease working with matrices over different types.
Converting `MultiStage::flat_distance_matrix` from `Matrix<usize>` into `Matrix<u8>`.
git commit -m "Reduced memory usage of multistage topologies."
Converted `dragonfly::distance_matrix` to `u8`.

## [0.2.0]

### 2021-03-12
git commit -m "Preparing to publish version 0.2."
git commit -m "Track multistage.rs"

### 2021-03-10
Added plugs for stages.
Attributes of `LevelRequirements` made public.
Removed from the `Topology` interfaz the never used methods `num_arcs`, `average_distance`, `distance_distribution`.
git commit -m "Added multistage topologies. Cleanup on Topology interfaz."
Added method `up_down_distance` to `Topology`.
Splitting up/down distance table in MultiStage into a up-component and a down-component. Removed its pure up-distance table.
New routing `UpDown`.
Replaced several `.expect(&format!(...))` by `.wrap_or_else(|_|panic!(...))`, to avoid formatting strings except when reporting errors.
Added a bit of documentation.

### 2021-03-09
Changed `WidenedStage` to use a boxed `base` as to be able to build it.
Added a `new` method to each stage.

### 2021-03-05
MultiStage sizes computed via LevelRequirements.
New stages ExplicitStage, WidenedStage.

### 2021-03-03
New file multistage.rs definining MultiStage topologies in terms of Stages connecting pairs of levels of routers.
Projective types Geometry, SelfDualGeometry, FlatGeometry, and FlatGeometryCache made public. And used in multistage for the OFT.
Added requirement FlatGeometry:Clone.
Implemented stages FatStage and ProjectiveStage, upon which the topologies XGFT and OFT are built.

### 2021-03-02
git tag 0.2.0 -m "v0.2.0"
git commit -m "tag to v0.2"

### 2021-02-12
Updating documentation.
Set version 0.2.0 in Cargo.toml.

### 2021-02-09
git commit -m "implemented Hotspots and RandomMix patterns."
Added the `at` config-function to access arrays in output description files.
git commit -m "Fix on RandomMix probability. Added the at config-function."

### 2021-02-03
git commit -m "Self-messages in Burst traffics now substract a pending message, allowing completion when there are fixed points in the pattern."

### 2021-02-01
Fixed 2021 dates in this changelog...
Correctly manage self-messages in burst traffic.
git commit -m "Correctly manage self-messages in burst traffic. Improvements on tikz backend."

### 2021-01-28
Completed `Stubborn::update_routing_info`, which had the recursion over its sub-routing missing.
git commit -m "Fixed Stubborn routing. Show journal messages."
Moved tikz back externalization plots from `externalization` to `externalization-plots`.
Protected the tikz backend externalization against some collisions.

### 2021-01-27
Added a `cycle` value to the result files. For the sake of burst simulations.
git commit -m "Added cycle to the result files."
Show journal messages with every action.

### 2021-01-25
Added dependence on crate procfs.
Report status.vmhwm, stat.utime, and stat.stime in the result.
git commit -m "Report process status at the end. Improved style of the tikz backend."

### 2021-01-12
A few color changes in the tikz backend.

### 2020-12-22
Added more colors, pens, and marks to tikz backend.

### 2020-12-21
Fixed a bug in ValiantDOR where the DOR part were sometimes non-minimal.
git commit -m "Fixed a bug in ValiantDOR where the DOR part were sometimes non-minimal."

### 2020-12-18
Added check to detect overflowing output buffers.
git commit -m "Added check to detect overflowing output buffers."

### 2020-12-16
Externalization fixes.

### 2020-12-15
Externalization of legends moved to a different folder.
Fixed bubble to actually reserve space for the current packet plus a maximum packet size.
git commit -m "Fixed bubble to actually reserve space for the current packet plus a maximum packet size."

### 2020-12-14
git commit -m "Enabled tikz externalization. Let main.cfg handle close."

### 2020-12-11
Enabled tikz externalization.
Added a prefix member to Plots.

### 2020-12-10
Added `ExperimentOptions::message`, intended to be used with `--message=text`, to be written into the journal file.
Removed unnecessary mut requirement of `Experiment::write_journal_entry`.
Removed quotes from the config `LitStr` and `Literal`.
git commit -m "Added shifts to CartesianTransform. Added a message option. Removed surrounding quotes of parsed literals."
git commit -m "Actually removed quoted from compiled grammar."
git commit -m "Added quotes when printing literals."
git commit -m "Removed quotes around git_id when building a literal."
Added enum BackendError and improved error managing on output generation.

### 2020-12-09
Added shift argument to CartesianTransform.
Renamed CartesianTornado as CartesianFactor. I messed up, this is not a generalization of tornado but something else entirely. The tornado pattern is just a shift by `(side-1)*0.5`, which can be written as `CartesianTransform{sides:[whole],shift:[halfside]}`, with `whole=side*side` and `halfside=(side-1)/2`.
Added `O1TURN::reserved_virtual_channels_order{01,10}` parameters to control the usage of virtual channels.

### 2020-12-07
Implemented the CartesianTornado pattern.
git commit -m "Implemented the CartesianTornado pattern."

### 2020-12-04
Added patterns `Composition` and `Pow`.
git commit -m "Added neighbour_router_iter to topologies to avoid misusing degree. Added patterns Composition and Pow."

### 2020-12-03
Ordering code on NeighboursLists.
Added `Topology::{write_adjacencies_to_file,neighbour_router_iter}`.
Removed `non_exhaustive` from TopologyBuilderArgument.
Use `neighbour_router_iter` always instead of `0..degree()`. `degree`  does not give valid ranges when having non-connected ports.

### 2020-12-01
Added the config if, add functions.
Allow to use "legend_name" directly in the simulation config root. This helps to use named experiment indices.
git commit -m "Added configuration functions"

### 2020-11-30
Added to the grammar function calls. To be used as "=functionname{key1:expr1, key2:expr2,}".
Added the config function `lt`.
Added member `ExperimentOptions::where_clause` to receive --where parameters.
The output::evaluate funtion made public.
Added `config_parser::parse_expression` to be used to capture the --where=expr clause.
git commit -m "Improved grammar: added named experiments and function calls."
New file config.rs to encapsulate all the processing of ConfigurationValue and expressions.
git commit -m "Moved config-processing aspects into new file config.rs."
Fixed bugs while calculating and showing statistics of the Basic router.
git commit -m "Fixed bugs while calculating and showing statistics of the Basic router."
Set pgfplots option `scaled ticks=false`.
Added Plotkind option `array`. It differs from histogram in that it does not normalize.

### 2020-11-27
Added 'named experiments' to the grammar. This is, to use `key: expa![val1,val2,val3]` and in other place `other_key: expa![wok1,wok2,wok3]`. Intended to get the matches `[{key:val1,other_key_wok1},{key:val2,other_key_wok2},{key:val3,other_key_wok3}]` instead of the whole of combinations.
Changed `flatten_configuration_value` to expand named experiments correctly.

### 2020-11-26
Added methods `Routing::{statistics,reset_statistics}` and `Router::{aggregate_statistics,reset_statistics}` to gather specific statistics of each type.
Added routing annotations.
Added method `Routing::performed_request` to allow routings to make decisions when the router makes a request to a candidate.
Implemented a Stubborn meta routing, what always repeat the same request over and over.
Added `SumRoutingPolicy::TryBoth`.
git commit -m "Added statistics to routings and routers. Routers now inform routings of the candidate they finally request."
git commit -m "Divided occpation in statistics by number of ports."
git commit -m "Added extra label parameter to SumRouting."
git commit -m "Fixed annotation on SumRouting."
git commit -m "More fixes on SumRouting."

## [0.1.0] 

### 2020-11-24
git tag 0.1.0 -m "v0.1.0"
git commit -m "Updated metdata for publication."

### 2020-11-23
Changed `Topology::coordinated_routing_record` to optionally receive a random number generator.
The torus topology now uses the random number generator to generate fair routing records to the opposing location for even sides.
git commit -m "Balanced routng records for torus."

### 2020-11-19

Implemented `{Mesh,Torus}::diameter`.
git commit -m "Provided diameter for meshes and tori."
New member `CandidateEgress::router_allows: Option<bool>` to capture whether the router consider the egress to satisfy the flow-control.
Moved pre-request checking of flow-control into a new `EnforceFlowControl` policy.
git commit -m "Moved pre-request checking of flow-control into a new EnforceFlowControl policy."

### 2020-11-18

git commit -m "Added slimfly and proyective topologies for prime fields."

### 2020-11-12

Some fixes for topologies with non-connected ports.
Got the projective topology implemented.
Also implemented the LeviProjective and the SlimFly topologies.
`Topology::check_adjacency_consistency` now also optionally checks a given number of link classes.
Added Quantifiable to `[T;2]`.

### 2020-11-11

More documentation.
Code cleanup.
git commit -m "First commit in the new caminos-lib git."
Begining to write the projective networks.

### 2020-11-09

Created repository `caminos-lib` with content copied from a private version.
Split into `caminos-lib` and `caminos`.
Created CHANGELOG.md and README.md
And using now edition=2018.

