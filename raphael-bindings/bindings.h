#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

enum class Action : uint8_t {
  BasicSynthesis,
  BasicTouch,
  MasterMend,
  Observe,
  TricksOfTheTrade,
  WasteNot,
  Veneration,
  StandardTouch,
  GreatStrides,
  Innovation,
  WasteNot2,
  ByregotsBlessing,
  PreciseTouch,
  MuscleMemory,
  CarefulSynthesis,
  Manipulation,
  PrudentTouch,
  AdvancedTouch,
  Reflect,
  PreparatoryTouch,
  Groundwork,
  DelicateSynthesis,
  IntensiveSynthesis,
  TrainedEye,
  HeartAndSoul,
  PrudentSynthesis,
  TrainedFinesse,
  RefinedTouch,
  QuickInnovation,
  ImmaculateMend,
  TrainedPerfection,
};

struct SolveArgs {
  void (*on_start)(bool*);
  void (*on_finish)(const Action*, size_t);
  void (*on_suggest_solution)(const Action*, size_t);
  void (*on_progress)(size_t);
  uint64_t action_mask;
  uint16_t progress;
  uint16_t quality;
  uint16_t base_progress;
  uint16_t base_quality;
  int16_t cp;
  int8_t durability;
  uint8_t job_level;
  bool adversarial;
  bool backload_progress;
  bool unsound_branch_pruning;
};

extern "C" {

void solve(const SolveArgs *args);

}  // extern "C"
