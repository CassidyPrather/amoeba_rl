using AmoebaRL.Core.Organelles;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Interfaces
{
    public interface IUpgradable
    {
        bool Upgrade(CraftingMaterial.Resource material);
    }
}
