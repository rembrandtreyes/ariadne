using System;
using System.Collections.Generic;

namespace MyApp
{
    public interface IUserService
    {
        void CreateUser(string name);
    }

    public class UserService : IUserService
    {
        public void CreateUser(string name)
        {
            ValidateName(name);
            Console.WriteLine("Created: " + name);
        }

        private void ValidateName(string name)
        {
            if (string.IsNullOrEmpty(name))
                throw new ArgumentException("Name required");
        }
    }

    public enum UserRole
    {
        Admin,
        User,
        Guest
    }

    class Program
    {
        static void Main(string[] args)
        {
            var service = new UserService();
            service.CreateUser("Alice");
        }
    }
}
