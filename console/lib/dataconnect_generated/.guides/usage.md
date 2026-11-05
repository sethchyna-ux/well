# Basic Usage

```dart
ExampleConnector.instance.CreateUser().execute();
ExampleConnector.instance.UpdateUser(updateUserVariables).execute();
ExampleConnector.instance.DeleteUser().execute();
ExampleConnector.instance.GetCurrentUser().execute();
ExampleConnector.instance.ListAllUsers().execute();
ExampleConnector.instance.CreateProfile(createProfileVariables).execute();
ExampleConnector.instance.UpdateProfile(updateProfileVariables).execute();
ExampleConnector.instance.DeleteProfile(deleteProfileVariables).execute();
ExampleConnector.instance.GetProfile(getProfileVariables).execute();
ExampleConnector.instance.ListMyProfiles().execute();

```

## Optional Fields

Some operations may have optional fields. In these cases, the Flutter SDK exposes a builder method, and will have to be set separately.

Optional fields can be discovered based on classes that have `Optional` object types.

This is an example of a mutation with an optional field:

```dart
await ExampleConnector.instance.UpdateDeviceSync({ ... })
.profileId(...)
.execute();
```

Note: the above example is a mutation, but the same logic applies to query operations as well. Additionally, `createMovie` is an example, and may not be available to the user.

