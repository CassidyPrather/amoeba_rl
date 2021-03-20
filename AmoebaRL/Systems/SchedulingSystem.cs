using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using AmoebaRL.Interfaces;

namespace AmoebaRL.Systems
{
    /// <summary>
    /// <see cref="https://faronbracy.github.io/RogueSharp/articles/15_scheduling_system.html"/>
    /// </summary>
    public class SchedulingSystem
    {
        private int _time;
        private readonly SortedDictionary<int, List<ISchedulable>> _scheduleables;

        public SchedulingSystem()
        {
            _time = 0;
            _scheduleables = new SortedDictionary<int, List<ISchedulable>>();
        }

        // Add a new object to the schedule
        // Place it at the current time plus the object's Time property.
        public void Add(ISchedulable scheduleable)
        {
            int key = _time + scheduleable.Time;
            if (!_scheduleables.ContainsKey(key))
            {
                _scheduleables.Add(key, new List<ISchedulable>());
            }
            _scheduleables[key].Add(scheduleable);
        }

        // Remove a specific object from the schedule.
        // Useful for when an monster is killed to remove it before it's action comes up again.
        public void Remove(ISchedulable scheduleable)
        {
            KeyValuePair<int, List<ISchedulable>> scheduleableListFound
              = new KeyValuePair<int, List<ISchedulable>>(-1, null);

            foreach (var scheduleablesList in _scheduleables)
            {
                if (scheduleablesList.Value.Contains(scheduleable))
                {
                    scheduleableListFound = scheduleablesList;
                    break;
                }
            }
            if (scheduleableListFound.Value != null)
            {
                scheduleableListFound.Value.Remove(scheduleable);
                if (scheduleableListFound.Value.Count <= 0)
                {
                    _scheduleables.Remove(scheduleableListFound.Key);
                }
            }
        }

        // Get the next object whose turn it is from the schedule. Advance time if necessary
        public ISchedulable Get()
        {
            var firstScheduleableGroup = _scheduleables.First();
            var firstScheduleable = firstScheduleableGroup.Value.First();
            Remove(firstScheduleable);
            _time = firstScheduleableGroup.Key;
            return firstScheduleable;
        }

        // Get the current time (turn) for the schedule
        public int GetTime()
        {
            return _time;
        }

        /// <summary>
        /// Get the time an <see cref="ISchedulable"/> is going to go.
        /// </summary>
        /// <param name="timeFor"></param>
        /// <returns></returns>
        public int? ScheduledFor(ISchedulable timeFor)
        {
            KeyValuePair<int, List<ISchedulable>> scheduleableListFound
                = new KeyValuePair<int, List<ISchedulable>>(-1, null);

            foreach (var scheduleablesList in _scheduleables)
            {
                if (scheduleablesList.Value.Contains(timeFor))
                {
                    scheduleableListFound = scheduleablesList;
                    break;
                }
            }
            if (scheduleableListFound.Value != null)
            {
                return scheduleableListFound.Key;
            }
            return null;
        }

        // Reset the time and clear out the schedule
        public void Clear()
        {
            _time = 0;
            _scheduleables.Clear();
        }
    }
}
