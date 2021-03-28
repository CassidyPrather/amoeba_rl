using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace AmoebaRL.Core
{
    /// <summary>
    /// Two-dimensional immutable integer vector.
    /// </summary>
    /// <remarks>Nomenclature inspired by JackNine</remarks>
    public struct Coord
    {
        /// <summary>
        /// Instantiates a new two-dimensional vector for (<see cref="X"/>, <see cref="Y"/>).
        /// </summary>
        /// <param name="x">The horizontal, or first, value of the vector.</param>
        /// <param name="y">The vertical, or second, value of the vector.</param>
        public Coord(int x = 0, int y = 0)
        {
            X = x;
            Y = y;
        }

        /// <summary>
        /// First value. Commonly the horizontal coordinate.
        /// </summary>
        public int X { get; set; }

        /// <summary>
        /// Second value. Commonly the vertical coordinate.
        /// </summary>
        public int Y { get; set; }

        /// <summary>
        /// { <see cref="X"/>, <see cref="Y"/> }
        /// </summary>
        public int[] Val => new int[] { X, Y };

        /// <summary>
        /// Determines the distance between this and <paramref name="other"/> using exclusively orthogonal steps.
        /// </summary>
        /// <param name="other">The <see cref="Coord"/> to take the distance with respect to.</param>
        /// <returns>The distance between this and  <paramref name="other"/> taken using exclusively orthogonal steps.</returns>
        public int TaxiDistance(Coord other) => Math.Abs(X - other.X) + Math.Abs(Y - other.Y);

        /// <summary>
        /// <c>sqrt(<see cref="X"/>^2 + <see cref="Y"/>^2)</c>
        /// </summary>
        /// <returns><c>sqrt(<see cref="X"/>^2 + <see cref="Y"/>^2)</c></returns>
        public double Magnitude() => Math.Sqrt(Math.Pow(X, 2) + Math.Pow(Y, 2));

        /// <summary>
        /// The componentwise sum of <paramref name="a"/> and <paramref name="b"/>.
        /// </summary>
        /// <param name="a">First set of coefficients.</param>
        /// <param name="b">Second set of coefficients.</param>
        /// <returns>The componentwise sum of <paramref name="a"/> and <paramref name="b"/>.</returns>
        public static Coord operator +(Coord a, Coord b) => new Coord(a.X + b.X, a.Y + b.Y);

        /// <summary>
        /// The componentwise difference of <paramref name="a"/> and <paramref name="b"/>.
        /// </summary>
        /// <param name="a">First set of coefficients.</param>
        /// <param name="b">Second set of coefficients.</param>
        /// <returns>The componentwise difference of <paramref name="a"/> and <paramref name="b"/>.</returns>
        public static Coord operator -(Coord a, Coord b) => new Coord(a.X - b.X, a.Y - b.Y);

        /// <summary>
        /// Mutiplies each component of <paramref name="a"/> by <paramref name="b"/>.
        /// </summary>
        /// <param name="a">The set of coefficients to multiply.</param>
        /// <param name="b">The scalar value to multiply each component of <paramref name="a"/> by.</param>
        /// <returns>The <see cref="Coord"/> with each component of <paramref name="a"/> multiplied by <paramref name="b"/>.</returns>
        public static Coord operator *(Coord a, int b) => new Coord(a.X * b, a.Y  * b);
    }
}
