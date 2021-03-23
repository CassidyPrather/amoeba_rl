using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    /// <summary>
    /// Can be added to <see cref="Systems.SchedulingSystem"/>.
    /// </summary>
    public interface ISchedulable
    {
        /// <summary>
        /// The amount of time past <see cref="Systems.SchedulingSystem.GetTime"/> to add this to the schedule.
        /// </summary>
        int Time { get; }
    }

    /// <summary>
    /// Has a behavior to enact after it is added to <see cref="Systems.SchedulingSystem"/>.
    /// </summary>
    public interface IPostSchedule : ISchedulable
    {
        /// <summary>
        /// Called after the instance is added to <see cref="Systems.SchedulingSystem"/>.
        /// </summary>
        void DoPostSchedule();
    }
}
